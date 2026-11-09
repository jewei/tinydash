package settings

import (
	"fmt"
	"strings"
)

// Accelerator turns a shortcut as settings.json stores it, in the Rust
// app's Tauri syntax ("Control+Shift+Space", "Super+Comma", "Alt+Numpad1"),
// into mygo's accelerator syntax ("Control+Shift+Space", "Super+,",
// "Alt+Num1"). The file keeps the Tauri form, so going back to the Rust
// app still reads it.
func Accelerator(stored string) (string, error) {
	parts := strings.Split(stored, "+")
	for i, part := range parts {
		if part == "" {
			return "", fmt.Errorf("%q is not a shortcut.", stored)
		}
		last := i == len(parts)-1
		if !last {
			if !modifiers[part] {
				return "", fmt.Errorf("%q in %q is not Control, Alt, Shift, or Super.", part, stored)
			}
			continue
		}
		key, ok := keyName(part)
		if !ok {
			return "", fmt.Errorf("TinyDash cannot register the %s key of %q.", part, stored)
		}
		parts[i] = key
	}
	return strings.Join(parts, "+"), nil
}

var modifiers = map[string]bool{"Control": true, "Alt": true, "Shift": true, "Super": true}

// signs are the keys macOS stored by their position, named as W3C codes.
var signs = map[string]string{
	"Backquote": "`", "Backslash": `\`, "BracketLeft": "[", "BracketRight": "]",
	"Comma": ",", "Equal": "=", "Minus": "-", "Period": ".", "Quote": "'",
	"Semicolon": ";", "Slash": "/",
}

var numpad = map[string]string{
	"Add": "NumAdd", "Subtract": "NumSub", "Multiply": "NumMult", "Divide": "NumDiv", "Decimal": "NumDec",
}

var named = map[string]bool{
	"Space": true, "Enter": true, "Tab": true, "Backspace": true, "Delete": true, "Insert": true,
	"Home": true, "End": true, "PageUp": true, "PageDown": true, "Up": true, "Down": true,
	"Left": true, "Right": true, "PrintScreen": true, "ScrollLock": true, "NumLock": true,
}

func keyName(key string) (string, bool) {
	switch {
	case len(key) == 1 && (key >= "A" && key <= "Z" || key >= "0" && key <= "9"):
		return key, true
	case named[key]:
		return key, true
	case signs[key] != "":
		return signs[key], true
	}
	if n, ok := strings.CutPrefix(key, "F"); ok && functionKey(n) {
		return key, true
	}
	if rest, ok := strings.CutPrefix(key, "Numpad"); ok {
		if len(rest) == 1 && rest >= "0" && rest <= "9" {
			return "Num" + rest, true
		}
		if name, ok := numpad[rest]; ok {
			return name, true
		}
	}
	return "", false // NumpadEnter and NumpadEqual have no mygo name
}

func functionKey(n string) bool {
	switch len(n) {
	case 1:
		return n >= "1" && n <= "9"
	case 2:
		return n[0] == '1' && n[1] >= '0' && n[1] <= '9' || n[0] == '2' && n[1] >= '0' && n[1] <= '4'
	}
	return false
}
