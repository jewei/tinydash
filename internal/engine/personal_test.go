package engine

import (
	"context"
	"errors"
	"fmt"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
)

func TestAHotkeyNeedsFreeKeysAndAnItemThatNeverAsks(t *testing.T) {
	w := newWorld(t, nil)
	check := func(id core.ID, keys string) (core.ID, string, string) {
		got, shortcut, err := checkedHotkey(w.snapshot(), w.keys, id, keys)
		return got, shortcut, errorText(err)
	}
	if id, keys, err := check("system:lock", " Alt+L "); id != "system:lock" || keys != "Alt+L" || err != "" {
		t.Errorf("%s %q %s", id, keys, err)
	}
	if _, _, err := check("system:lock", "Control+Shift+Space"); err != "These keys show TinyDash. Choose other keys." {
		t.Error(err)
	}
	if _, _, err := check("system:lock", "Alt+Nothing"); err == "" {
		t.Error("an unknown key")
	}
	// Plain keys would stop that key from typing in every app.
	for _, keys := range []string{"KeyA", "Shift+KeyA", "Shift+Space"} {
		if _, _, err := check("system:lock", keys); err == "" {
			t.Error(keys)
		}
	}
	for _, keys := range []string{"F5", "Super+KeyL"} {
		if _, _, err := check("system:lock", keys); err != "" {
			t.Errorf("%s: %s", keys, err)
		}
	}
	if _, _, err := check("system:sleep", "Alt+S"); err != "Sleep asks before it runs, so it cannot have a hotkey." {
		t.Error(err)
	}
	if _, _, err := check("clip:1", "Alt+C"); err != "This result cannot have a hotkey." {
		t.Error(err)
	}
	gone := core.SourceApp.ID(golden.Abs("/Gone.app"))
	if _, _, err := check(gone, "Alt+G"); err != "This item no longer exists." {
		t.Error(err)
	}
	// A gone item's hotkey can still be removed.
	if id, keys, err := check(gone, ""); id != gone || keys != "" || err != "" {
		t.Errorf("%s %q %s", id, keys, err)
	}

	// The same keys, written another way, belong to Lock Screen.
	w.edit(func(next *search.Snapshot) {
		next.Hotkeys = search.NewHotkeys(map[core.ID]string{"system:lock": "Alt+KeyL"})
	})
	if _, _, err := check("system:open-trash", "alt+l"); err != "These keys already run Lock Screen. Choose other keys." {
		t.Error(err)
	}
	if _, _, err := check("system:lock", "Alt+L"); err != "" {
		t.Error(err)
	}

	full := map[core.ID]string{}
	for n := range search.MaxHotkeys {
		full[core.SourceApp.ID(golden.Abs(fmt.Sprintf("/%d.app", n)))] = fmt.Sprintf("Alt+F%d", n%24+1)
	}
	w.edit(func(next *search.Snapshot) { next.Hotkeys = search.NewHotkeys(full) })
	if _, _, err := check("system:lock", "Alt+Shift+L"); !strings.HasPrefix(err, "You can set up to 50 hotkeys.") {
		t.Error(err)
	}
}

func TestAHotkeyRegistersBeforeItIsSaved(t *testing.T) {
	w := newWorld(t, nil)
	lock := core.ID("system:lock")
	taken := errors.New("Could not register Alt+L.")
	w.keys.failing = map[core.ID]error{lock: taken}
	if err := w.run(core.SetHotkey{ID: lock, Shortcut: "Alt+L"}, ""); err != taken {
		t.Fatalf("%v", err)
	}
	// The keys that did not register are released again.
	if n := len(w.keys.synced); n != 2 || w.keys.synced[1].Len() != 0 {
		t.Errorf("synced %v", w.keys.synced)
	}
	saved, _ := w.db.Hotkeys(context.Background())
	if len(saved) != 0 || w.snapshot().Hotkeys.Len() != 0 {
		t.Errorf("saved keys that did not register: %v", saved)
	}

	w.keys.failing = nil
	if err := w.run(core.SetHotkey{ID: lock, Shortcut: "Alt+L"}, ""); err != nil {
		t.Fatal(err)
	}
	saved, _ = w.db.Hotkeys(context.Background())
	if keys, _ := w.snapshot().Hotkeys.Get(lock); saved[lock] != "Alt+L" || keys != "Alt+L" {
		t.Errorf("%v %q", saved, keys)
	}
	if err := w.run(core.SetHotkey{ID: lock, Shortcut: ""}, ""); err != nil {
		t.Fatal(err)
	}
	saved, _ = w.db.Hotkeys(context.Background())
	if len(saved) != 0 || w.snapshot().Hotkeys.Len() != 0 {
		t.Errorf("a removed hotkey stays: %v", saved)
	}
}

func TestAHotkeyThatIsOffAtStartupNamesItsResult(t *testing.T) {
	w := newWorld(t, nil)
	off := func(id core.ID) string {
		return HotkeyOff(w.snapshot(), id, errors.New("Could not register Alt+L."))
	}
	if got := off("system:lock"); got != "The hotkey of Lock Screen is off. Could not register Alt+L. Use Set Hotkey in its actions, or remove it in Settings > Search." {
		t.Error(got)
	}
	notes := golden.Abs("/Users/me/notes.txt")
	if got := off(core.SourceFile.ID(notes)); !strings.HasPrefix(got, "The hotkey of "+notes+" is off.") {
		t.Error(got)
	}
}

func TestAHotkeyPressedBeforeTheFirstScanAsksToWait(t *testing.T) {
	w := newWorld(t, nil)
	gone := "This hotkey's item no longer exists. Remove the hotkey in Settings > Search."
	waiting := "TinyDash is still loading your apps and files. Try again in a moment."
	other := core.SourceApp.ID(golden.Abs("/Applications/Other.app"))
	notes := core.SourceFile.ID(golden.Abs("/notes.txt"))
	errorFor := func(id core.ID, scanned bool) string {
		s := w.snapshot()
		s.AppsReady, s.FilesReady = scanned, scanned
		_, err := hotkeyAction(s, id)
		return errorText(err)
	}
	for _, c := range []struct {
		id      core.ID
		scanned bool
		want    string
	}{
		{other, false, waiting},
		{notes, false, waiting},
		{other, true, gone},
		// Commands need no scan, so a gone one is gone at once.
		{"system:gone", false, gone},
		{"system:sleep", true, "This item asks before it runs. Run it from TinyDash."},
	} {
		if got := errorFor(c.id, c.scanned); got != c.want {
			t.Errorf("%s: %s", c.id, got)
		}
	}
	if action, err := hotkeyAction(w.snapshot(), "system:lock"); err != nil || action != (core.System{Command: core.CommandLock}) {
		t.Errorf("%v %v", action, err)
	}
	if err := w.actions.RunHotkey(core.SourceApp.ID(safari)); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "launch /Applications/Safari.app", "hide")
}

func TestAFullPinListGivesUpItsOldestHiddenPin(t *testing.T) {
	w := newWorld(t, nil)
	emoji := ids(search.Search(w.snapshot(), "", core.CategoryEmoji))
	if len(emoji) != search.MaxPins {
		t.Fatalf("%d emoji", len(emoji))
	}
	newApp := core.SourceApp.ID(golden.Abs("/New.app"))
	gone := core.SourceFile.ID(golden.Abs("/gone"))
	drop := func(pins []core.ID, hidden []core.ID, scanned bool) (core.ID, bool, error) {
		s := w.snapshot()
		s.Pins, s.Hidden = search.NewPins(pins), search.NewHidden(hidden...)
		s.AppsReady, s.FilesReady = scanned, scanned
		return pinToDrop(s, newApp)
	}
	if _, dropped, err := drop(emoji[1:], nil, true); dropped || err != nil {
		t.Errorf("room left: %v %v", dropped, err)
	}
	// History is off, so the clipboard pin is hidden too, but newer.
	pins := append([]core.ID{gone, "clip:7"}, emoji[2:]...)
	if oldest, _, err := drop(pins, nil, true); oldest != gone || err != nil {
		t.Errorf("%s %v", oldest, err)
	}
	// Before the first scan, the file pin may only look hidden.
	if oldest, _, err := drop(pins, nil, false); oldest != "clip:7" || err != nil {
		t.Errorf("%s %v", oldest, err)
	}
	if _, _, err := drop(emoji, nil, true); errorText(err) != "You can pin up to 100 items." {
		t.Errorf("%v", err)
	}
	// A pin made before its item was hidden shows in no list.
	if oldest, _, err := drop(emoji, emoji[3:4], true); oldest != emoji[3] || err != nil {
		t.Errorf("%s %v", oldest, err)
	}
	// A second Pin of a pinned item changes nothing.
	s := w.snapshot()
	s.Pins = search.NewPins(emoji)
	if _, dropped, err := pinToDrop(s, emoji[5]); dropped || err != nil {
		t.Errorf("%v %v", dropped, err)
	}

	// Pin drops the pin from the list and the database.
	ctx := context.Background()
	for _, pin := range pins {
		if err := w.db.SetPinned(ctx, pin, true); err != nil {
			t.Fatal(err)
		}
	}
	w.edit(func(next *search.Snapshot) { next.Pins = search.NewPins(pins) })
	if err := w.run(core.Pin{ID: "system:lock"}, ""); err != nil {
		t.Fatal(err)
	}
	saved, _ := w.db.Pins(ctx)
	if got := w.snapshot().Pins.IDs(); !slices.Equal(got, saved) || slices.Contains(got, gone) || got[len(got)-1] != "system:lock" {
		t.Errorf("pins %v, saved %v", got, saved)
	}
}

func ids(results []core.Result) []core.ID {
	out := make([]core.ID, len(results))
	for i, r := range results {
		out[i] = r.ID
	}
	return out
}

func TestOnlyExistingItemsAreHiddenAndHiddenOnesAreNotPinned(t *testing.T) {
	w := newWorld(t, nil)
	lock := core.ID("system:lock")
	if err := w.run(core.Pin{ID: lock}, ""); err != nil {
		t.Fatal(err)
	}
	if err := w.run(core.Hide{ID: lock}, ""); err != nil {
		t.Fatal(err)
	}
	if s := w.snapshot(); !s.Hidden.Contains(lock) || s.Pins.Contains(lock) {
		t.Error("a hidden item stays pinned")
	}
	if err := w.run(core.Pin{ID: lock}, ""); !strings.Contains(errorText(err), "hidden") {
		t.Errorf("%v", err)
	}
	ctx := context.Background()
	if pins, _ := w.db.Pins(ctx); len(pins) != 0 || w.snapshot().Pins.Len() != 0 {
		t.Errorf("pins %v", pins)
	}
	for _, id := range []core.ID{"system:gone", "snippet:99"} {
		if err := w.run(core.Hide{ID: id}, ""); err == nil {
			t.Error(id)
		}
	}
	if hidden, _ := w.db.Hidden(ctx); !slices.Equal(hidden, []core.ID{lock}) || w.snapshot().Hidden.Len() != 1 {
		t.Errorf("hidden %v", hidden)
	}
}

func TestAnAliasNamesOneExistingResult(t *testing.T) {
	w := newWorld(t, withLibrary)
	lock := core.ID("system:lock")
	if err := w.run(core.SetAlias{ID: lock, Alias: " Lk "}, ""); err != nil {
		t.Fatal(err)
	}
	if alias, _ := w.snapshot().Aliases.Get(lock); alias != "lk" {
		t.Errorf("%q", alias)
	}
	if err := w.run(core.SetAlias{ID: "system:sleep", Alias: "lk"}, ""); !strings.Contains(errorText(err), "Lock Screen") {
		t.Errorf("%v", err)
	}
	for _, id := range []core.ID{"system:gone", "clip:1"} {
		if err := w.run(core.SetAlias{ID: id, Alias: "x"}, ""); err == nil {
			t.Error(id)
		}
	}
	snippet := core.SourceSnippet.ID("1")
	if err := w.run(core.SetAlias{ID: snippet, Alias: "sg"}, ""); err != nil {
		t.Fatal(err)
	}
	if owner, _ := w.snapshot().Aliases.Owner("sg"); owner != snippet {
		t.Errorf("%s", owner)
	}
	if err := w.run(core.SetAlias{ID: snippet, Alias: ""}, ""); err != nil {
		t.Fatal(err)
	}
	if _, ok := w.snapshot().Aliases.Owner("sg"); ok {
		t.Error("a removed alias stays")
	}
	if top := search.Search(w.snapshot(), "lk", core.CategoryAll); top[0].ID != lock {
		t.Errorf("%s", top[0].ID)
	}
	if err := w.run(core.SetAlias{ID: lock, Alias: ""}, ""); err != nil {
		t.Fatal(err)
	}
	if saved, _ := w.db.Aliases(context.Background()); len(saved) != 0 || w.snapshot().Aliases.Len() != 0 {
		t.Errorf("%v", saved)
	}
}

func TestAFullAliasListRefusesNewAliasesButAllowsRenames(t *testing.T) {
	w := newWorld(t, nil)
	lock := core.ID("system:lock")
	full := map[core.ID]string{lock: "lk"}
	for n := 1; n < search.MaxAliases; n++ {
		full[core.SourceApp.ID(golden.Abs(fmt.Sprintf("/%d.app", n)))] = fmt.Sprintf("a%d", n)
	}
	w.edit(func(next *search.Snapshot) { next.Aliases = search.NewAliases(full) })
	if err := w.run(core.SetAlias{ID: "system:sleep", Alias: "sl"}, ""); !strings.Contains(errorText(err), "Remove some") {
		t.Errorf("%v", err)
	}
	if err := w.run(core.SetAlias{ID: lock, Alias: "lock"}, ""); err != nil {
		t.Fatal(err)
	}
	s := w.snapshot()
	if alias, _ := s.Aliases.Get(lock); alias != "lock" || s.Aliases.Len() != search.MaxAliases {
		t.Errorf("%q, %d aliases", alias, s.Aliases.Len())
	}
}

func TestAnAliasOfAGoneItemCanStillBeRemoved(t *testing.T) {
	w := newWorld(t, nil)
	gone := core.SourceApp.ID(golden.Abs("/Gone.app"))
	ctx := context.Background()
	if err := w.db.SetAlias(ctx, gone, "gn"); err != nil {
		t.Fatal(err)
	}
	w.edit(func(next *search.Snapshot) {
		next.Aliases = search.NewAliases(map[core.ID]string{gone: "gn"})
	})
	if err := w.run(core.SetAlias{ID: gone, Alias: ""}, ""); err != nil {
		t.Fatal(err)
	}
	if saved, _ := w.db.Aliases(ctx); len(saved) != 0 || w.snapshot().Aliases.Len() != 0 {
		t.Errorf("%v", saved)
	}
}

// An ID of no source, such as one of the Rust app's shell commands, gets
// nothing that the user keeps.
func TestAnIDOfNoSourceIsNeverPinnedHiddenAliasedOrGivenAHotkey(t *testing.T) {
	w := newWorld(t, nil)
	id := core.ID("shell:git pull")
	for _, action := range []core.Action{
		core.Pin{ID: id}, core.Hide{ID: id}, core.SetAlias{ID: id, Alias: "gp"}, core.SetHotkey{ID: id, Shortcut: "Alt+G"},
	} {
		if err := w.run(action, ""); err == nil {
			t.Errorf("%#v", action)
		}
	}
	s := w.snapshot()
	if s.Pins.Len()+s.Hidden.Len()+s.Aliases.Len()+s.Hotkeys.Len() != 0 || len(w.keys.synced) != 0 {
		t.Error("a list changed")
	}
}
