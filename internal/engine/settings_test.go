package engine

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
)

func TestUpdateSettingsSavesAndPublishes(t *testing.T) {
	e, _ := start(t)
	dir := t.TempDir()
	_, _, file := LoadSettings(dir)
	if err := <-e.UpdateSettings(file, func(s *settings.Settings) { s.HideOnBlur = false }); err != nil {
		t.Fatal(err)
	}
	if e.current.Load().Settings.HideOnBlur {
		t.Fatal("not published")
	}
	data, _ := os.ReadFile(filepath.Join(dir, settings.FileName))
	if !strings.Contains(string(data), `"hideOnBlur": false`) {
		t.Fatalf("not saved:\n%s", data)
	}
}

// Turning history off in Settings is a switch-off like any other: content
// read before it is never saved after it.
func TestTurningHistoryOffInSettingsDropsAStaleSave(t *testing.T) {
	e, _ := start(t)
	_, _, file := LoadSettings(t.TempDir())
	epoch := e.ClipEpoch()
	<-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = false })
	<-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = true })
	<-e.SaveClip(epoch, clipboard.Text("secret"), 100)
	if n := clips(t, e); n != 0 {
		t.Fatalf("%d clips from before history was off", n)
	}
}

func TestASettingsSaveThatFailsChangesNothing(t *testing.T) {
	e, changes := start(t)
	file := SettingsFile{dir: t.TempDir(), replaceable: false}
	if err := <-e.UpdateSettings(file, func(s *settings.Settings) { s.HideOnBlur = false }); err == nil {
		t.Fatal("a refused save passed")
	}
	if !e.current.Load().Settings.HideOnBlur || changes.Load() != 0 {
		t.Fatal("a failed save changed the published settings")
	}
}

// system records how the Settings window's changes reach the OS. fail
// refuses a change to that shortcut.
type system struct {
	applied []string
	fail    string
}

func (s *system) apply(from, to settings.Settings) error {
	if from.Shortcut == to.Shortcut {
		return nil
	}
	s.applied = append(s.applied, to.Shortcut)
	if to.Shortcut == s.fail {
		return errors.New("taken")
	}
	return nil
}

// Ports update_settings and apply_to_system of commands.rs.
func TestASettingsChangeAppliesToTheSystemThenSaves(t *testing.T) {
	w := newWorld(t, nil)
	_, _, file := LoadSettings(t.TempDir())
	sys := &system{}
	if err := w.actions.UpdateSettings(file, func(s *settings.Settings) { s.Shortcut = "Alt+Space" }, sys.apply); err != nil {
		t.Fatal(err)
	}
	if w.snapshot().Settings.Shortcut != "Alt+Space" || !slices.Equal(sys.applied, []string{"Alt+Space"}) {
		t.Errorf("shortcut %q, applied %q", w.snapshot().Settings.Shortcut, sys.applied)
	}
}

func TestASettingsChangeThatFailsIsUndoneInTheSystem(t *testing.T) {
	w := newWorld(t, nil)
	_, _, file := LoadSettings(t.TempDir())
	sys := &system{fail: "Alt+Space"}
	if err := w.actions.UpdateSettings(file, func(s *settings.Settings) { s.Shortcut = "Alt+Space" }, sys.apply); errorText(err) != "taken" {
		t.Fatal(err)
	}
	// The old shortcut registers again.
	if w.snapshot().Settings.Shortcut != "Control+Shift+Space" || !slices.Equal(sys.applied, []string{"Alt+Space", "Control+Shift+Space"}) {
		t.Errorf("shortcut %q, applied %q", w.snapshot().Settings.Shortcut, sys.applied)
	}

	sys = &system{}
	refused := SettingsFile{dir: t.TempDir(), replaceable: false}
	if err := w.actions.UpdateSettings(refused, func(s *settings.Settings) { s.Shortcut = "Alt+Space" }, sys.apply); err == nil {
		t.Fatal("a refused save passed")
	}
	if w.snapshot().Settings.Shortcut != "Control+Shift+Space" || !slices.Equal(sys.applied, []string{"Alt+Space", "Control+Shift+Space"}) {
		t.Errorf("after a refused save: shortcut %q, applied %q", w.snapshot().Settings.Shortcut, sys.applied)
	}
}

func TestRefusesSettingsThatCannotWork(t *testing.T) {
	w := newWorld(t, nil)
	w.edit(func(next *search.Snapshot) {
		next.Hotkeys = search.NewHotkeys(map[core.ID]string{"system:lock": "Alt+L"})
	})
	_, _, file := LoadSettings(t.TempDir())
	for _, c := range []struct {
		change func(*settings.Settings)
		want   string
	}{
		{func(s *settings.Settings) { s.FileSearchFolders = append(s.FileSearchFolders, "Projects") }, "“Projects” is not a full folder path."},
		{func(s *settings.Settings) { s.ClockCities = []string{"Atlantis"} }, "TinyDash does not know “Atlantis”."},
		{func(s *settings.Settings) { s.Shortcut = "Alt+L" }, "These keys already run Lock Screen."},
		{func(s *settings.Settings) { s.Shortcut = "Shift+KeyA" }, "never blocks typing"},
	} {
		sys := &system{}
		if err := w.actions.UpdateSettings(file, c.change, sys.apply); !strings.Contains(errorText(err), c.want) {
			t.Errorf("%v, want %q", err, c.want)
		}
		if len(sys.applied) != 0 {
			t.Errorf("a refused change reached the system: %q", sys.applied)
		}
	}
	if err := w.actions.UpdateSettings(file, func(s *settings.Settings) { s.Shortcut = "Shift+Space" }, nil); err != nil {
		t.Error(err)
	}
}

func TestEmojiLanguagesAndRatesApplyAtOnce(t *testing.T) {
	w := newWorld(t, nil)
	_, _, file := LoadSettings(t.TempDir())
	before := w.snapshot().Emoji
	err := w.actions.UpdateSettings(file, func(s *settings.Settings) {
		s.EmojiLanguages = []settings.EmojiLanguage{"ms"}
		s.CurrencyRatesEnabled = true
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if w.snapshot().Emoji == before {
		t.Error("the emoji index kept its languages")
	}
	if err := w.db.SaveRates(context.Background(), "{}"); err != nil {
		t.Fatal(err)
	}
	w.edit(func(next *search.Snapshot) { next.Rates = &currency.Rates{} })
	if err := w.actions.UpdateSettings(file, func(s *settings.Settings) { s.CurrencyRatesEnabled = false }, nil); err != nil {
		t.Fatal(err)
	}
	if _, ok, _ := w.db.Rates(context.Background()); ok || w.snapshot().Rates != nil {
		t.Error("rates that are off are kept")
	}
}

// Lowering the limit removes the oldest entries at once, as the Rust app
// did, even while history is off: they are saved, and come back when it is
// on again.
func TestLoweringTheClipboardLimitTrimsAtOnceEvenWhileHistoryIsOff(t *testing.T) {
	e, _ := start(t)
	_, _, file := LoadSettings(t.TempDir())
	for _, text := range []string{"one", "two", "three"} {
		if err := <-e.SaveClip(e.ClipEpoch(), clipboard.Text(text), 100); err != nil {
			t.Fatal(err)
		}
	}
	<-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = false })
	if err := <-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryLimit = 1 }); err != nil {
		t.Fatal(err)
	}
	<-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = true })
	entries := e.Snapshot(e.current.Load().Now).Clipboard.Browse(clipboard.Context{})
	if len(entries) != 1 || entries[0].Title != "three" {
		t.Fatalf("after lowering the limit to 1 with history off: %d entries %v, want only the newest", len(entries), entries)
	}
}
