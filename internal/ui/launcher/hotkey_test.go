package launcher

import (
	"slices"
	"testing"

	"github.com/egoist/mygo/ui"
)

// Ports acceleratorFromEvent and displayKeys of src/lib/keys.ts.
func TestAccelerator(t *testing.T) {
	for _, c := range []struct {
		name  string
		ev    ui.InputEvent
		onMac bool
		want  string
	}{
		{"modifiers in Tauri's order", ui.InputEvent{Key: ui.KeyK, Mods: ui.Super | ui.Shift | ui.Alt | ui.Ctrl}, false, "Control+Alt+Shift+Super+K"},
		{"a digit", ui.InputEvent{Key: ui.Key7, Mods: ui.Ctrl}, false, "Control+7"},
		{"a letter needs a strong modifier", ui.InputEvent{Key: ui.KeyK, Mods: ui.Shift}, false, ""},
		{"Enter alone is no hotkey", ui.InputEvent{Key: ui.KeyEnter}, false, ""},
		{"a function key alone", ui.InputEvent{Key: ui.KeyF5}, false, "F5"},
		{"F13 by position", ui.InputEvent{Code: ui.CodeF13}, true, "F13"},
		{"F24 off macOS", ui.InputEvent{Code: ui.CodeF24}, false, "F24"},
		{"no F21 on macOS", ui.InputEvent{Code: ui.CodeF21}, true, ""},
		{"Shift+Space", ui.InputEvent{Key: ui.KeySpace, Mods: ui.Shift}, false, "Shift+Space"},
		{"an arrow", ui.InputEvent{Key: ui.KeyUp, Mods: ui.Alt}, false, "Alt+Up"},
		{"a lone modifier", ui.InputEvent{Code: ui.CodeShiftLeft, Mods: ui.Shift}, true, ""},
		// macOS registers keys by position: AZERTY's A key is Q on a US keyboard.
		{"macOS reads the position", ui.InputEvent{Key: ui.KeyA, PhysicalKey: ui.KeyQ, Mods: ui.Super}, true, "Super+Q"},
		{"elsewhere the layout's letter", ui.InputEvent{Key: ui.KeyA, PhysicalKey: ui.KeyQ, Mods: ui.Ctrl}, false, "Control+A"},
		{"Cyrillic: the letter's position", ui.InputEvent{PhysicalKey: ui.KeyS, Mods: ui.Ctrl}, true, "Control+S"},
		{"signs by position on macOS", ui.InputEvent{Key: ui.KeySlash, PhysicalKey: ui.KeySlash, Mods: ui.Ctrl}, true, "Control+Slash"},
		{"no signs elsewhere", ui.InputEvent{Key: ui.KeySlash, Mods: ui.Ctrl}, false, ""},
		// Windows AltGr comes as Control+Alt and types no Key.
		{"AltGr+7 types {", ui.InputEvent{Code: ui.CodeDigit7, Mods: ui.Ctrl | ui.Alt}, false, ""},
		{"a numpad digit", ui.InputEvent{Key: ui.Key5, Code: ui.CodeNumpad5, Mods: ui.Ctrl}, false, "Control+Numpad5"},
		{"a numpad digit with Num Lock off", ui.InputEvent{Key: ui.KeyEnd, Code: ui.CodeNumpad1, Mods: ui.Ctrl}, false, ""},
		{"numpad Enter registers as another key off macOS", ui.InputEvent{Key: ui.KeyEnter, Code: ui.CodeNumpadEnter, Mods: ui.Ctrl}, false, ""},
		{"numpad Enter on macOS", ui.InputEvent{Key: ui.KeyEnter, Code: ui.CodeNumpadEnter, Mods: ui.Super}, true, "Super+NumpadEnter"},
		{"Scroll Lock off macOS", ui.InputEvent{Code: ui.CodeScrollLock, Mods: ui.Ctrl}, false, "Control+ScrollLock"},
		{"no Scroll Lock on macOS", ui.InputEvent{Code: ui.CodeScrollLock, Mods: ui.Super}, true, ""},
	} {
		if got := accelerator(c.ev, c.onMac); got != c.want {
			t.Errorf("%s: %q, want %q", c.name, got, c.want)
		}
	}
}

func TestDisplayKeys(t *testing.T) {
	for _, c := range []struct {
		accelerator string
		onMac       bool
		superKey    string
		want        []string
	}{
		{"Control+Shift+Space", true, "Super", []string{"⌃", "⇧", "Space"}},
		{"Control+Shift+Space", false, "Super", []string{"Ctrl", "Shift", "Space"}},
		{"Super+Alt+K", true, "Super", []string{"⌘", "⌥", "K"}},
		{"Super+Alt+K", false, "Win", []string{"Win", "Alt", "K"}},
		{"CommandOrControl+Up", false, "Super", []string{"Ctrl", "↑"}},
		{"", true, "Super", nil},
	} {
		if got := displayKeys(c.accelerator, c.onMac, c.superKey); !slices.Equal(got, c.want) {
			t.Errorf("%q mac %v: %q, want %q", c.accelerator, c.onMac, got, c.want)
		}
	}
}
