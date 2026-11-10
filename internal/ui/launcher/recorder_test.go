package launcher

import (
	"errors"
	"slices"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// Recording pauses the launcher shortcut and every hotkey. Leaving the
// panel must end it, as the Rust recorder's blur did: else the shortcuts
// stay off while the user is in another app, and on macOS only Finder
// opens TinyDash again.
func TestRecordingStopsWhenThePanelLosesFocus(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	m, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	tt.SetFocused(false)
	if m.hotkey == nil || m.hotkey.rec.On() {
		t.Fatalf("dialog %+v", m.hotkey)
	}
	if !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("paused %v", f.paused)
	}
	// Back in the panel, a click records again.
	tt.SetFocused(true)
	click(t, tt, "Hotkey for Safari: None")
	if !m.hotkey.rec.On() || !slices.Equal(f.paused, []bool{true, false, true}) {
		t.Errorf("recording %v, paused %v", m.hotkey.rec.On(), f.paused)
	}
}

// A panel can hide without a blur reaching the view first, as the
// shortcut or Escape in another dialog hides it: the app says so.
func TestHidingThePanelStopsRecording(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	m, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	m.Hidden()
	tt.Frame()
	if m.hotkey.rec.On() || !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("recording %v, paused %v", m.hotkey.rec.On(), f.paused)
	}
}

// A save that runs as the panel hides keeps the shortcuts paused until it
// ends, so the app registers the saved keys.
func TestHidingThePanelDuringAHotkeySaveResumesAfterIt(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	f.hold = true
	m, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	tt.Key(ui.Ctrl|ui.Alt, ui.KeyK)
	m.Hidden()
	if !slices.Equal(f.paused, []bool{true}) {
		t.Fatalf("paused %v during the save", f.paused)
	}
	f.finish(nil)
	if !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("paused %v", f.paused)
	}
}

// The dialog keeps the save's own reason: a resume error is about other
// keys.
func TestAResumeErrorDoesNotReplaceTheSavesReason(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	f.hold = true
	_, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	tt.Key(ui.Ctrl|ui.Alt, ui.KeyK)
	f.pauseErr = errors.New("Could not register Control+Shift+Space: taken.")
	f.finish(errors.New("Could not register Control+Alt+K: taken."))
	tt.Frame()
	if !tt.HasText("Could not register Control+Alt+K: taken.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// Each recording shows its own result: a refused save's reason goes when
// the next recording starts, so a refused resume after it can show.
func TestANewRecordingClearsTheLastReason(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	f.fail = func(core.Action) error { return errors.New("Could not register Control+Alt+K: taken.") }
	m, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	tt.Key(ui.Ctrl|ui.Alt, ui.KeyK)
	tt.Frame()
	if !tt.HasText("Could not register Control+Alt+K: taken.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	click(t, tt, "Hotkey for Safari: None")
	tt.Frame()
	if tt.HasText("Could not register Control+Alt+K: taken.") {
		t.Errorf("the last recording's reason stays: %q", tt.Texts())
	}
	f.pauseErr = errors.New("Could not register Control+Shift+Space: taken.")
	tt.Key(0, ui.KeyEscape)
	tt.Frame()
	if m.hotkey == nil || !tt.HasText("Could not register Control+Shift+Space: taken.") {
		t.Errorf("texts %q", tt.Texts())
	}
}
