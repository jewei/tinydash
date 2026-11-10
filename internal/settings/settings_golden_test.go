package settings

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/golden"
)

// dropped are the keys of the features the Go port leaves out. Load keeps
// them as they are.
var dropped = []string{
	"expandSnippets", "shellCommands", "showUpNext", "showNotepad", "showFocusTimer",
	"focusMinutes", "shortBreakMinutes", "longBreakMinutes", "sessionsBeforeLongBreak",
	"showClipboardCards",
}

type settingsRow struct {
	Name     string                     `json:"name"`
	Input    *string                    `json:"input"`
	Settings map[string]json.RawMessage `json:"settings"`
	Warning  *string                    `json:"warning"`
	Backup   bool                       `json:"backup"`
}

func TestSettingsMatchRust(t *testing.T) {
	rows := golden.Rows[settingsRow](t, "fixtures/settings")
	if len(rows) == 0 {
		t.Fatal("no rows")
	}
	for _, row := range rows {
		input := []byte(nil)
		if row.Input != nil {
			input = []byte(*row.Input)
		}
		loaded := Load(input, row.Input != nil)

		want := map[string]json.RawMessage{}
		for name, value := range row.Settings {
			want[name] = value
		}
		for _, name := range dropped {
			delete(want, name)
		}
		// The fixture was written on macOS, where the tray icon is off by
		// default.
		if !setsKey(row.Input, "showTrayIcon") {
			want["showTrayIcon"], _ = json.Marshal(Default().ShowTrayIcon)
		}
		modeled := loaded.Settings
		modeled.kept = nil
		var warning any
		if w := loaded.Warning("<dir>", loaded.Backup); w != "" {
			// The fixture was written on macOS; the backup path uses this
			// OS's separator, as the Rust app's did.
			warning = strings.ReplaceAll(w, "<dir>"+string(filepath.Separator), "<dir>/")
		}
		golden.Check(t, "settings", row.Name,
			mustJSON(t, map[string]any{"settings": want, "warning": row.Warning, "backup": row.Backup}),
			map[string]any{"settings": modeled, "warning": warning, "backup": loaded.Backup})

		if row.Warning != nil && loaded.Warning("<dir>", true) == "" {
			t.Errorf("%s: Rust warned, Go did not", row.Name)
		}
		// The keys Go does not model come back as the file had them, and
		// Rust read the same values from them.
		for name, value := range loaded.Settings.kept {
			if rust, ok := row.Settings[name]; ok && !jsonEqual(t, rust, value) {
				t.Errorf("%s: kept %s = %s, Rust read %s", row.Name, name, value, rust)
			}
		}
	}
}

func setsKey(input *string, key string) bool {
	var object map[string]json.RawMessage
	return input != nil && json.Unmarshal([]byte(*input), &object) == nil && object[key] != nil
}

func mustJSON(t *testing.T, v any) json.RawMessage {
	t.Helper()
	data, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func jsonEqual(t *testing.T, a, b json.RawMessage) bool {
	t.Helper()
	var x, y any
	if json.Unmarshal(a, &x) != nil || json.Unmarshal(b, &y) != nil {
		t.Fatalf("not JSON: %s, %s", a, b)
	}
	return string(mustJSON(t, x)) == string(mustJSON(t, y))
}
