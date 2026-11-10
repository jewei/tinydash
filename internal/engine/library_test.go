package engine

import (
	"context"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// Ports save_library_item and delete_library_item of commands.rs.

func TestSavesANewSnippetAndUpdatesIt(t *testing.T) {
	w := newWorld(t, nil)
	saved, err := w.actions.SaveLibraryItem(library.Item{Kind: library.KindSnippet, Name: " Greeting ", Keyword: "HI", Text: "Hello {query}"})
	if err != nil {
		t.Fatal(err)
	}
	if saved.ID == 0 || saved.Name != "Greeting" || saved.Keyword != "hi" {
		t.Fatalf("saved %+v", saved)
	}
	if found, ok := w.snapshot().Library.Find(saved.ID); !ok || found != saved {
		t.Fatalf("published %+v %v", found, ok)
	}
	saved.Text = "Hi {query}"
	again, err := w.actions.SaveLibraryItem(saved)
	if err != nil || again.ID != saved.ID {
		t.Fatalf("%+v %v", again, err)
	}
	items := w.snapshot().Library.Items()
	if len(items) != 1 || items[0].Text != "Hi {query}" {
		t.Errorf("items %+v", items)
	}
	if rows, _ := w.db.Library(context.Background()); len(rows) != 1 || rows[0].Text != "Hi {query}" {
		t.Errorf("stored %+v", rows)
	}
}

func TestRefusesALibraryItemThatCannotWork(t *testing.T) {
	w := newWorld(t, withLibrary)
	for _, c := range []struct {
		item library.Item
		want string
	}{
		{library.Item{Kind: library.KindSnippet, Name: " ", Text: "x"}, "Enter a name of up to 100 characters."},
		{library.Item{Kind: library.KindSnippet, Name: "A", Keyword: "two words", Text: "x"}, "A keyword is one word of up to 32 characters."},
		{library.Item{Kind: library.KindQuicklink, Name: "Run", Text: "file:///bin/sh"}, "A quicklink opens"},
		{library.Item{Kind: library.KindSnippet, Name: "Other", Keyword: "s", Text: "x"}, "The keyword “s” is used by Search."},
		{library.Item{ID: 1, Kind: library.KindQuicklink, Name: "Sig", Text: "https://x.test"}, "A snippet cannot become a quicklink"},
	} {
		if _, err := w.actions.SaveLibraryItem(c.item); !strings.Contains(errorText(err), c.want) {
			t.Errorf("%+v: %v, want %q", c.item, err, c.want)
		}
	}
	if n := len(w.snapshot().Library.Items()); n != 3 {
		t.Errorf("%d items after refused saves", n)
	}
}

func TestStopsAddingAtTheLibraryLimit(t *testing.T) {
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		items := make([]library.Item, library.MaxItems)
		for i := range items {
			items[i] = library.Item{ID: int64(i + 1), Kind: library.KindSnippet, Name: "S", Text: "x"}
		}
		first.Library = library.NewIndex(items)
	})
	_, err := w.actions.SaveLibraryItem(library.Item{Kind: library.KindSnippet, Name: "One more", Text: "x"})
	if errorText(err) != "The library holds up to 500 snippets and quicklinks." {
		t.Error(err)
	}
}

func TestDeletingAnItemTakesItsPersonalRowsAndHotkey(t *testing.T) {
	w := newWorld(t, withLibrary)
	sig := core.SourceSnippet.ID("1")
	for _, action := range []core.Action{core.Pin{ID: sig}, core.SetAlias{ID: sig, Alias: "sg"}, core.SetHotkey{ID: sig, Shortcut: "Alt+G"}} {
		if err := w.run(action, ""); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.actions.DeleteLibraryItem(1); err != nil {
		t.Fatal(err)
	}
	s := w.snapshot()
	if _, ok := s.Library.Find(1); ok || s.Pins.Contains(sig) || s.Aliases.Len() != 0 || s.Hotkeys.Len() != 0 {
		t.Errorf("left behind: pins %v aliases %v hotkeys %v", s.Pins.IDs(), s.Aliases.All(), s.Hotkeys.All())
	}
	if last := w.keys.synced[len(w.keys.synced)-1]; last.Len() != 0 {
		t.Errorf("the hotkey stays registered: %v", last.All())
	}
	if rows, _ := w.db.Library(context.Background()); len(rows) != 2 {
		t.Errorf("stored %+v", rows)
	}
}

func TestShowsAHiddenResultAgain(t *testing.T) {
	w := newWorld(t, nil)
	lock := core.ID("system:lock")
	if err := w.run(core.Hide{ID: lock}, ""); err != nil {
		t.Fatal(err)
	}
	if err := w.actions.Unhide(lock); err != nil {
		t.Fatal(err)
	}
	if w.snapshot().Hidden.Contains(lock) {
		t.Error("still hidden")
	}
	if hidden, _ := w.db.Hidden(context.Background()); len(hidden) != 0 {
		t.Errorf("stored %v", hidden)
	}
}

// Ports describe and hidden_results of commands.rs.
func TestListsHiddenResultsByTitle(t *testing.T) {
	gone, notes := safari+"x", golden.Abs("/Users/me/notes.txt")
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.Hidden = search.NewHidden("system:sleep", "system:lock", core.SourceApp.ID(gone), core.SourceFile.ID(notes))
		first.FilesReady = false
	})
	var got []string
	for _, r := range HiddenResults(w.snapshot()) {
		got = append(got, r.Title+" / "+r.Subtitle)
	}
	want := []string{gone + " / No longer found", notes + " / Still loading", "Lock Screen / Lock this computer", "Sleep / Put the computer to sleep"}
	if !slices.Equal(got, want) {
		t.Errorf("%q, want %q", got, want)
	}
}

// Ports aliased and hotkeys of commands.rs: aliases by alias, hotkeys by
// title, each with its result's title, or its key when the item is gone.
func TestListsAliasesAndHotkeys(t *testing.T) {
	gone := safari + "x"
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.Aliases = search.NewAliases(map[core.ID]string{"system:sleep": "zz", "system:lock": "lk", core.SourceApp.ID(gone): "old"})
		first.Hotkeys = search.NewHotkeys(map[core.ID]string{"system:sleep": "Control+Alt+S", "system:lock": "Control+Alt+L"})
	})
	list := func(rows []Listed) []string {
		var out []string
		for _, r := range rows {
			out = append(out, r.Value+" "+r.Title+" / "+r.Subtitle)
		}
		return out
	}
	if got, want := list(AliasResults(w.snapshot())), []string{"lk Lock Screen / Lock this computer", "old " + gone + " / No longer found", "zz Sleep / Put the computer to sleep"}; !slices.Equal(got, want) {
		t.Errorf("aliases %q, want %q", got, want)
	}
	if got, want := list(HotkeyResults(w.snapshot())), []string{"Control+Alt+L Lock Screen / Lock this computer", "Control+Alt+S Sleep / Put the computer to sleep"}; !slices.Equal(got, want) {
		t.Errorf("hotkeys %q, want %q", got, want)
	}
}
