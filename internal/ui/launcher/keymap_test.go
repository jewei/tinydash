package launcher

import (
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// The keymap port of src/launcher/keymap.test.ts, with what each key does
// to a launcher showing Safari, Notes, and a clipboard entry.
func TestKeymap(t *testing.T) {
	notes := app("Notes")
	for _, c := range []struct {
		name   string
		mods   ui.Modifiers
		key    ui.Key
		repeat bool
		before func(*Model)
		want   func(*Model, *fake) bool
	}{
		{"Down moves down", 0, ui.KeyDown, false, nil, func(m *Model, _ *fake) bool { return m.selected == 1 }},
		{"a held Down keeps moving", 0, ui.KeyDown, true, nil, func(m *Model, _ *fake) bool { return m.selected == 1 }},
		{"Down stops at the last row", 0, ui.KeyDown, false, func(m *Model) { m.selected = 2 }, func(m *Model, _ *fake) bool { return m.selected == 2 }},
		{"Up moves up", 0, ui.KeyUp, false, func(m *Model) { m.selected = 2 }, func(m *Model, _ *fake) bool { return m.selected == 1 }},
		{"Up on the first row brings back a search", 0, ui.KeyUp, false, func(m *Model) { m.history = []string{"saf"} }, func(m *Model, _ *fake) bool { return m.query == "saf" }},
		{"a held Up brings back nothing", 0, ui.KeyUp, true, func(m *Model) { m.history = []string{"saf"} }, func(m *Model, _ *fake) bool { return m.query == "" }},
		{"Enter runs the main action", 0, ui.KeyEnter, false, nil, func(_ *Model, f *fake) bool {
			return len(f.runs) == 1 && f.runs[0] == ran{safari.Actions[0].Action, safari.ID}
		}},
		{"a held Enter runs nothing", 0, ui.KeyEnter, true, nil, func(_ *Model, f *fake) bool { return len(f.runs) == 0 }},
		{"Mod+Enter runs the second action", ui.Cmd, ui.KeyEnter, false, nil, func(_ *Model, f *fake) bool {
			return len(f.runs) == 1 && f.runs[0] == ran{safari.Actions[1].Action, ""}
		}},
		{"Mod+2 runs the second row", ui.Cmd, ui.Key2, false, nil, func(_ *Model, f *fake) bool {
			return len(f.runs) == 1 && f.runs[0] == ran{notes.Actions[0].Action, notes.ID}
		}},
		{"Mod+9 past the rows runs nothing", ui.Cmd, ui.Key9, false, nil, func(_ *Model, f *fake) bool { return len(f.runs) == 0 }},
		{"Tab goes to the next tab", 0, ui.KeyTab, false, nil, func(m *Model, _ *fake) bool { return m.category == core.CategoryApps }},
		{"Shift+Tab goes round to the last tab", ui.Shift, ui.KeyTab, false, nil, func(m *Model, _ *fake) bool { return m.category == core.CategorySystem }},
		{"Mod+K opens the actions menu", ui.Cmd, ui.KeyK, false, nil, func(m *Model, _ *fake) bool { return m.menu != nil }},
		{"Mod+Backspace deletes a clipboard entry", ui.Cmd, ui.KeyBackspace, false, func(m *Model) { m.selected = 2 }, func(_ *Model, f *fake) bool {
			return len(f.runs) == 1 && f.runs[0] == ran{core.DeleteClip{ID: 1}, ""}
		}},
		{"Mod+, opens Settings", ui.Cmd, ui.KeyComma, false, nil, func(_ *Model, f *fake) bool {
			return len(f.runs) == 1 && f.runs[0] == ran{core.OpenSettings{}, ""}
		}},
		{"Escape clears the query", 0, ui.KeyEscape, false, func(m *Model) { m.setQuery("saf") }, func(m *Model, f *fake) bool { return m.query == "" && f.hidden == 0 }},
		{"Escape on an empty query hides", 0, ui.KeyEscape, false, nil, func(_ *Model, f *fake) bool { return f.hidden == 1 }},
		{"Escape closes the menu first", 0, ui.KeyEscape, false, func(m *Model) { m.setQuery("saf"); m.openMenu() }, func(m *Model, _ *fake) bool { return m.menu == nil && m.query == "saf" }},
	} {
		t.Run(c.name, func(t *testing.T) {
			f := newFake(always(safari, notes, clip))
			m := New(f)
			if c.before != nil {
				c.before(m)
			}
			if !m.press(c.mods, c.key, c.repeat) {
				t.Fatal("no binding took the key")
			}
			if !c.want(m, f) {
				t.Errorf("selected %d, query %q, category %q, menu %v, runs %+v, hidden %d",
					m.selected, m.query, m.category, m.menu != nil, f.runs, f.hidden)
			}
		})
	}
}

// Keys the keymap leaves keep their meaning in the search field.
func TestKeymapLeavesTyping(t *testing.T) {
	m := New(newFake(always(safari)))
	for _, k := range []struct {
		mods ui.Modifiers
		key  ui.Key
	}{{0, ui.KeyK}, {0, ui.Key3}, {ui.Cmd, ui.KeyBackspace}, {ui.Cmd, ui.KeyC}, {0, ui.KeyBackspace}} {
		if m.press(k.mods, k.key, false) {
			t.Errorf("the keymap took %v+%v", k.mods, k.key)
		}
	}
}

// A dialog over the launcher takes the keyboard: the launcher's keys wait.
func TestKeysBehindADialogWait(t *testing.T) {
	named := core.Result{ID: safari.ID, Kind: core.KindApp, Title: "Safari",
		Actions: []core.ResultAction{core.Do("Add Alias…", core.SetAlias{ID: safari.ID})}}
	f := newFake(always(named))
	_, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	tt.Key(ui.Cmd, ui.Key1)
	tt.Key(ui.Cmd, ui.KeyK)
	tt.Key(0, ui.KeyTab)
	wantRuns(t, f)
	if f.lastSearch().category != core.CategoryAll {
		t.Errorf("Tab behind the dialog changed the tab to %q", f.lastSearch().category)
	}
}

func TestHistoryKeepsEachQueryOnceUpToTheLimit(t *testing.T) {
	m := New(newFake(always()))
	for i := range historyLimit + 5 {
		m.remember(string(rune('a' + i%26)))
		m.remember(string(rune('A' + i)))
	}
	m.remember("a")
	if len(m.history) != historyLimit || m.history[0] != "a" {
		t.Fatalf("history %q", m.history)
	}
	seen := map[string]bool{}
	for _, q := range m.history {
		if seen[q] {
			t.Fatalf("%q twice in %q", q, m.history)
		}
		seen[q] = true
	}
}

func TestMenuFilterMatchesEveryWord(t *testing.T) {
	mn := &menu{items: []menuItem{{label: "Show in Finder"}, {label: "Open"}, {label: "Quit TinyDash"}}}
	for filter, want := range map[string][]string{
		"":            {"Show in Finder", "Open", "Quit TinyDash"},
		"show":        {"Show in Finder"},
		"FIND  show":  {"Show in Finder"},
		"tiny quit":   {"Quit TinyDash"},
		"open finder": nil,
		"zzz":         nil,
	} {
		mn.filter = filter
		var got []string
		for _, item := range mn.visible() {
			got = append(got, item.label)
		}
		if len(got) != len(want) || (len(got) > 0 && got[0] != want[0]) {
			t.Errorf("%q: %q, want %q", filter, got, want)
		}
	}
}
