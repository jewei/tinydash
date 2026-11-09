package launcher

import (
	"slices"
	"testing"

	"github.com/egoist/mygo/ui"
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
	if m.hotkey == nil || m.hotkey.recording {
		t.Fatalf("dialog %+v", m.hotkey)
	}
	if !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("paused %v", f.paused)
	}
	// Back in the panel, a click records again.
	tt.SetFocused(true)
	click(t, tt, "Hotkey for Safari: None")
	if !m.hotkey.recording || !slices.Equal(f.paused, []bool{true, false, true}) {
		t.Errorf("recording %v, paused %v", m.hotkey.recording, f.paused)
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
	if m.hotkey.recording || !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("recording %v, paused %v", m.hotkey.recording, f.paused)
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
