package settings

import (
	"bytes"
	"os"
	"testing"

	"github.com/egoist/mygo/ui"
)

// The recorder must read keys exactly as the launcher's hotkey dialog does,
// whose tests cover them; this keeps the two copies the same.
func TestKeysMatchTheLaunchers(t *testing.T) {
	ours, err := os.ReadFile("keys.go")
	if err != nil {
		t.Fatal(err)
	}
	theirs, err := os.ReadFile("../launcher/hotkey.go")
	if err != nil {
		t.Fatal(err)
	}
	_, ours, _ = bytes.Cut(ours, []byte("\nimport ("))
	_, theirs, _ = bytes.Cut(theirs, []byte("\nimport ("))
	if !bytes.Equal(ours, theirs) {
		t.Error("keys.go differs from internal/ui/launcher/hotkey.go")
	}
}

func TestRecordsTheLauncherShortcutAsSettingsStoreIt(t *testing.T) {
	for _, c := range []struct {
		ev    ui.InputEvent
		onMac bool
		want  string
	}{
		{ui.InputEvent{Key: ui.KeySpace, Mods: ui.Alt}, false, "Alt+Space"},
		{ui.InputEvent{Key: ui.KeySpace, Mods: ui.Shift}, false, "Shift+Space"},
		{ui.InputEvent{Key: ui.KeyK, Mods: ui.Shift}, false, ""},
		{ui.InputEvent{Key: ui.KeyA, PhysicalKey: ui.KeyQ, Mods: ui.Super}, true, "Super+Q"},
	} {
		if got := accelerator(c.ev, c.onMac); got != c.want {
			t.Errorf("%+v: %q, want %q", c.ev, got, c.want)
		}
	}
}
