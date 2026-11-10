package settings

import (
	"fmt"
	"testing"
)

// Every key the Rust app's recorder could store (src/lib/keys.ts: letters,
// digits, FIXED_KEYS, and macOS MAC_KEYS).
func TestEveryStoredKeyTranslates(t *testing.T) {
	want := map[string]string{
		"Control+Shift+Space":         "Control+Shift+Space",
		"Alt+K":                       "Alt+K",
		"Super+7":                     "Super+7",
		"Control+Alt+Shift+Super+F24": "Control+Alt+Shift+Super+F24",
		"Shift+Space":                 "Shift+Space",
		"F1":                          "F1",
		"Super+Comma":                 "Super+,",
		"Super+Backquote":             "Super+`",
		"Super+Backslash":             `Super+\`,
		"Alt+BracketLeft":             "Alt+[",
		"Alt+BracketRight":            "Alt+]",
		"Alt+Equal":                   "Alt+=",
		"Alt+Minus":                   "Alt+-",
		"Alt+Period":                  "Alt+.",
		"Alt+Quote":                   "Alt+'",
		"Alt+Semicolon":               "Alt+;",
		"Alt+Slash":                   "Alt+/",
		"Control+Numpad5":             "Control+Num5",
		"Control+NumpadAdd":           "Control+NumAdd",
		"Control+NumpadSubtract":      "Control+NumSub",
		"Control+NumpadMultiply":      "Control+NumMult",
		"Control+NumpadDivide":        "Control+NumDiv",
		"Control+NumpadDecimal":       "Control+NumDec",
		"Control+Up":                  "Control+Up",
		"Control+PageDown":            "Control+PageDown",
		"Control+PrintScreen":         "Control+PrintScreen",
		"Control+ScrollLock":          "Control+ScrollLock",
		"Control+NumLock":             "Control+NumLock",
	}
	for _, key := range []string{"Enter", "Tab", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "Down", "Left", "Right"} {
		want["Control+"+key] = "Control+" + key
	}
	for n := 1; n <= 24; n++ {
		want[fmt.Sprintf("Alt+F%d", n)] = fmt.Sprintf("Alt+F%d", n)
	}
	for stored, accelerator := range want {
		got, err := Accelerator(stored)
		if err != nil || got != accelerator {
			t.Errorf("%s: %q %v, want %q", stored, got, err, accelerator)
		}
	}
}

// Keys mygo cannot name fail with a message, rather than turning into
// another key.
func TestKeysWithoutAMygoNameFail(t *testing.T) {
	for _, stored := range []string{"Control+NumpadEnter", "Control+NumpadEqual", "Control+F25", "Control+F0", "Hyper+K", "Control+", "", "Control+a", "Control+KeyKK", "Control+Digit10"} {
		if got, err := Accelerator(stored); err == nil {
			t.Errorf("%q gave %q", stored, got)
		}
	}
}
