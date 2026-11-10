package kit

import (
	"strings"

	"github.com/egoist/mygo/ui"
)

// keyNames are the keys besides letters and digits a hotkey can use, by
// their names in Tauri accelerators, as settings store them. Escape is not
// here: it stops recording.
var keyNames = map[ui.Key]string{
	ui.KeySpace: "Space", ui.KeyEnter: "Enter", ui.KeyTab: "Tab", ui.KeyBackspace: "Backspace",
	ui.KeyDelete: "Delete", ui.KeyInsert: "Insert", ui.KeyHome: "Home", ui.KeyEnd: "End",
	ui.KeyPageUp: "PageUp", ui.KeyPageDown: "PageDown",
	ui.KeyUp: "Up", ui.KeyDown: "Down", ui.KeyLeft: "Left", ui.KeyRight: "Right",
	ui.KeyF1: "F1", ui.KeyF2: "F2", ui.KeyF3: "F3", ui.KeyF4: "F4", ui.KeyF5: "F5", ui.KeyF6: "F6",
	ui.KeyF7: "F7", ui.KeyF8: "F8", ui.KeyF9: "F9", ui.KeyF10: "F10", ui.KeyF11: "F11", ui.KeyF12: "F12",
}

// signNames are sign keys, which macOS registers by their position. Windows
// and X11 find them by codes that differ between layouts, so only macOS
// takes them.
var signNames = map[ui.Key]string{
	ui.KeyBackquote: "Backquote", ui.KeyBackslash: "Backslash", ui.KeyBracketLeft: "BracketLeft",
	ui.KeyBracketRight: "BracketRight", ui.KeyComma: "Comma", ui.KeyEqual: "Equal",
	ui.KeyMinus: "Minus", ui.KeyPeriod: "Period", ui.KeyQuote: "Quote",
	ui.KeySemicolon: "Semicolon", ui.KeySlash: "Slash",
}

// codeNames are keys that only their position names: the numpad, F13 and
// up, and Print Screen.
var codeNames = map[ui.KeyCode]string{
	ui.CodeNumpad0: "Numpad0", ui.CodeNumpad1: "Numpad1", ui.CodeNumpad2: "Numpad2",
	ui.CodeNumpad3: "Numpad3", ui.CodeNumpad4: "Numpad4", ui.CodeNumpad5: "Numpad5",
	ui.CodeNumpad6: "Numpad6", ui.CodeNumpad7: "Numpad7", ui.CodeNumpad8: "Numpad8",
	ui.CodeNumpad9: "Numpad9", ui.CodeNumpadAdd: "NumpadAdd", ui.CodeNumpadSubtract: "NumpadSubtract",
	ui.CodeNumpadMultiply: "NumpadMultiply", ui.CodeNumpadDivide: "NumpadDivide",
	ui.CodeNumpadDecimal: "NumpadDecimal",
	ui.CodeF13:           "F13", ui.CodeF14: "F14", ui.CodeF15: "F15", ui.CodeF16: "F16",
	ui.CodeF17: "F17", ui.CodeF18: "F18", ui.CodeF19: "F19", ui.CodeF20: "F20",
	ui.CodePrintScreen: "PrintScreen",
}

// platformCodes hold the keys one side registers and the other does not:
// off macOS, global-hotkey registers numpad Enter and = and Num Lock as
// other keys, and macOS cannot register Scroll Lock or F21 and up. ""
// refuses the key.
var platformCodes = map[bool]map[ui.KeyCode]string{
	true: {ui.CodeNumpadEnter: "NumpadEnter", ui.CodeNumpadEqual: "NumpadEqual", ui.CodeNumLock: "NumLock", ui.CodeScrollLock: "",
		ui.CodeF21: "", ui.CodeF22: "", ui.CodeF23: "", ui.CodeF24: ""},
	false: {ui.CodeNumpadEnter: "", ui.CodeNumpadEqual: "", ui.CodeNumLock: "", ui.CodeScrollLock: "ScrollLock",
		ui.CodeF21: "F21", ui.CodeF22: "F22", ui.CodeF23: "F23", ui.CodeF24: "F24"},
}

// Accelerator is a hotkey in Tauri accelerator syntax, such as
// "Control+Alt+K", or "" for a press that is not one: a lone modifier, a
// key no OS registers the same way, or keys without Control, Alt, or
// Super, which would block typing. Function keys and Shift+Space need no
// modifier. macOS registers keys by position, so onMac reads letters and
// digits where a US keyboard has them; Windows and X11 by what the layout
// types. A key held with AltGr comes as no Key, so it is never a hotkey.
func Accelerator(ev ui.InputEvent, onMac bool) string {
	key := ev.Key
	// PhysicalKey is unknown where the platform does not report it.
	if onMac && ev.PhysicalKey != ui.KeyUnknown {
		key = ev.PhysicalKey
	}
	name := keyName(key, ev.Code, onMac)
	if name == "" {
		return ""
	}
	strong := ev.Mods&(ui.Ctrl|ui.Alt|ui.Super) != 0
	function := len(name) > 1 && name[0] == 'F' && name[1] >= '1' && name[1] <= '9'
	if !strong && !function && !(ev.Mods&ui.Shift != 0 && name == "Space") {
		return ""
	}
	var parts []string
	for _, mod := range []struct {
		mod  ui.Modifiers
		name string
	}{{ui.Ctrl, "Control"}, {ui.Alt, "Alt"}, {ui.Shift, "Shift"}, {ui.Super, "Super"}} {
		if ev.Mods&mod.mod != 0 {
			parts = append(parts, mod.name)
		}
	}
	return strings.Join(append(parts, name), "+")
}

func keyName(key ui.Key, code ui.KeyCode, onMac bool) string {
	if name, ok := platformCodes[onMac][code]; ok {
		return name
	}
	// The numpad comes by position first: with Num Lock off, Windows sends
	// End or Delete for its digits, and the shortcut would not fire.
	if name, ok := codeNames[code]; ok {
		if !onMac && strings.HasPrefix(name, "Numpad") && key != ui.KeyUnknown && !isDigitOrSign(key) {
			return ""
		}
		return name
	}
	switch {
	case key >= ui.KeyA && key <= ui.KeyZ:
		return string(rune('A' + key - ui.KeyA))
	case key >= ui.Key0 && key <= ui.Key9:
		return string(rune('0' + key - ui.Key0))
	}
	if name, ok := keyNames[key]; ok {
		return name
	}
	if onMac {
		return signNames[key]
	}
	return ""
}

func isDigitOrSign(key ui.Key) bool {
	_, sign := signNames[key]
	return key >= ui.Key0 && key <= ui.Key9 || sign
}

var macSymbols = map[string]string{
	"Control": "⌃", "Ctrl": "⌃", "Alt": "⌥", "Option": "⌥", "Shift": "⇧",
	"Super": "⌘", "Command": "⌘", "Cmd": "⌘", "CommandOrControl": "⌘", "CmdOrCtrl": "⌘",
	"Up": "↑", "Down": "↓", "Left": "←", "Right": "→", "Enter": "↩", "Backspace": "⌫",
}

var otherNames = map[string]string{
	"Control": "Ctrl", "CommandOrControl": "Ctrl", "CmdOrCtrl": "Ctrl",
	"Up": "↑", "Down": "↓", "Left": "←", "Right": "→",
}

// DisplayKeys are an accelerator's keys as the desktop names them, such as
// ⌃ ⇧ Space on macOS and Ctrl Shift Space elsewhere.
func DisplayKeys(accelerator string, onMac bool, superKey string) []string {
	var out []string
	for _, part := range strings.Split(accelerator, "+") {
		switch {
		case part == "":
		case onMac:
			out = append(out, lookup(macSymbols, part))
		case part == "Super" || part == "Command" || part == "Cmd":
			out = append(out, superKey)
		default:
			out = append(out, lookup(otherNames, part))
		}
	}
	return out
}

func lookup(names map[string]string, part string) string {
	if name, ok := names[part]; ok {
		return name
	}
	return part
}
