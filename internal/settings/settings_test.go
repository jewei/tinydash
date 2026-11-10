package settings

import (
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"reflect"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func load(t *testing.T, text string) Loaded {
	t.Helper()
	return Load([]byte(text), true)
}

func TestChangesReplaceOnlyTheirFields(t *testing.T) {
	old := Default()
	old.LaunchAtLogin = true
	changed, err := old.WithChanges(map[string]json.RawMessage{"clipboardHistoryEnabled": json.RawMessage("true")})
	if err != nil || !changed.ClipboardHistoryEnabled || !changed.LaunchAtLogin {
		t.Fatalf("%+v %v", changed, err)
	}
	if _, err := old.WithChanges(map[string]json.RawMessage{"appearance": json.RawMessage(`"sage"`)}); err == nil {
		t.Error("an unknown setting was accepted")
	}
	_, err = old.WithChanges(map[string]json.RawMessage{"hideOnBlur": json.RawMessage("false"), "theme": json.RawMessage(`"sage"`)})
	if err == nil || !strings.Contains(err.Error(), "“theme”") {
		t.Errorf("error %v names no theme", err)
	}
}

func TestADraggedPositionSavesAndClearsLikeAnySetting(t *testing.T) {
	moved, err := Default().WithChanges(map[string]json.RawMessage{"launcherPosition": json.RawMessage(`{"x": 40.5, "y": -900.0}`)})
	if err != nil || !reflect.DeepEqual(moved.LauncherPosition, &Position{40.5, -900}) {
		t.Fatalf("%v %v", moved.LauncherPosition, err)
	}
	reset, err := moved.WithChanges(map[string]json.RawMessage{"launcherPosition": json.RawMessage("null")})
	if err != nil || reset.LauncherPosition != nil {
		t.Fatalf("%v %v", reset.LauncherPosition, err)
	}
}

func TestAnUnreadableTabIsDroppedWithoutResettingTheRest(t *testing.T) {
	loaded := load(t, `{"hideOnBlur": false, "tabs": [{"category": "notes", "shown": true},
		{"category": "emoji", "shown": false}, "clipbord"]}`)
	backup := filepath.Join("/settings", BackupName)
	want := "Some values under these names in settings.json could not be read: tabs. " +
		"TinyDash skipped only those values and uses defaults for them. The old file is at " + backup + "."
	if got := loaded.Warning("/settings", true); got != want {
		t.Errorf("warning %q", got)
	}
	if !loaded.Backup || loaded.Damaged || !loaded.Replaceable(false) {
		t.Errorf("a file with an unreadable value is copied aside and stays replaceable: %+v", loaded)
	}
	if !strings.HasSuffix(loaded.Warning("/settings", false), "uses defaults for them.") {
		t.Error("a copy that failed must not be named")
	}
	s := loaded.Settings
	if s.HideOnBlur || s.Tabs[0] != (Tab{core.CategoryEmoji, false, true}) || len(s.Tabs) != len(core.Categories())-1 {
		t.Errorf("%+v", s)
	}
}

func TestAValueFromANewerVersionTakesItsDefaultWithoutResettingTheRest(t *testing.T) {
	loaded := load(t, `{"shortcut": "Alt+Space", "searchEngine": "kagi",
		"emojiLanguages": ["zh", "xx-unknown"], "focusMinutes": "soon"}`)
	warning := loaded.Warning("/settings", true)
	for _, field := range []string{"emojiLanguages", "searchEngine", "focusMinutes"} {
		if !strings.Contains(warning, field) {
			t.Errorf("%q names no %s", warning, field)
		}
	}
	if strings.Contains(warning, "shortcut") {
		t.Errorf("%q", warning)
	}
	s := loaded.Settings
	if s.Shortcut != "Alt+Space" || s.SearchEngine != Default().SearchEngine ||
		!slices.Equal(s.EmojiLanguages, []EmojiLanguage{"zh"}) || s.FocusMinutes != Default().FocusMinutes {
		t.Errorf("%+v", s)
	}
}

func TestDamagedFilesAreMovedAside(t *testing.T) {
	for _, text := range []string{"[1, 2]", "[]", "null", "42", `"text"`, "{ not json", `{"a": 1} {}`, "{\"clockCities\": [\"S\xe3o Paulo\"]}"} {
		loaded := load(t, text)
		if !loaded.Damaged || !loaded.Backup || !reflect.DeepEqual(loaded.Settings, Default()) {
			t.Errorf("%q: %+v", text, loaded)
		}
		if w := loaded.Warning("/settings", true); !strings.HasPrefix(w, "Settings were invalid (") || !strings.Contains(w, BackupName) {
			t.Errorf("%q: warning %q", text, w)
		}
		// Without a backup, a save would put defaults over the only copy.
		if loaded.Replaceable(false) || !strings.Contains(loaded.Warning("/settings", false), "will not be saved") {
			t.Errorf("%q: a damaged file that was not moved aside must never be replaced", text)
		}
		if !loaded.Replaceable(true) {
			t.Errorf("%q: once moved aside, the file can be replaced", text)
		}
	}
	for text, want := range map[string]string{
		"":                       "line 1 column 0",
		`{"theme": `:             "line 1 column 10",
		"{\n  \"theme\": tru\n}": "line 2 column 15",
		"{\n  \"theme\": x}":     "line 2 column 12",
	} {
		if got := load(t, text).Warning("/s", true); !strings.Contains(got, want) {
			t.Errorf("%q: %q has no %q", text, got, want)
		}
	}
}

func TestAbsentAndCleanFilesNeedNoWarning(t *testing.T) {
	for _, loaded := range []Loaded{Load(nil, false), load(t, "{}"), load(t, `{"hideOnBlur": false}`)} {
		if loaded.Backup || loaded.Damaged || loaded.Warning("/s", true) != "" || !loaded.Replaceable(false) {
			t.Errorf("%+v", loaded)
		}
	}
}

func TestAFileThatCouldNotBeReadIsNeverReplaced(t *testing.T) {
	warning := ReadFailedWarning(errors.New("locked"))
	if !strings.Contains(warning, "(locked)") || !strings.Contains(warning, "will not be saved") {
		t.Error(warning)
	}
}

// serde read each field into a typed struct: a value of the wrong type, a
// null, or a number that does not fit is unreadable.
func TestValuesMustHaveTheirExactType(t *testing.T) {
	cases := []struct {
		text     string
		readable bool
	}{
		{`{"shortcut": null}`, false},
		{`{"shortcut": 1}`, false},
		{`{"hideOnBlur": 0}`, false},
		{`{"clipboardHistoryLimit": 250.0}`, false},
		{`{"clipboardHistoryLimit": 1e2}`, false},
		{`{"clipboardHistoryLimit": -1}`, false},
		{`{"clipboardHistoryLimit": -0}`, true},
		{`{"clipboardHistoryLimit": 4294967295}`, true},
		{`{"clipboardHistoryLimit": 4294967296}`, false},
		{`{"emojiSkinTone": 255}`, true},
		{`{"emojiSkinTone": 256}`, false},
		{`{"fileSearchFolders": ["a", null]}`, false},
		{`{"fileSearchFolders": "a"}`, false},
		{`{"launcherPosition": null}`, true},
		{`{"launcherPosition": {"x": 1}}`, false},
		{`{"launcherPosition": {"x": 1, "y": 2, "z": 3}}`, true},
		{`{"tabs": [{"category": "apps", "shown": true, "inAll": null}]}`, false},
		{`{"tabs": [{"category": "apps", "shown": true, "extra": 1}]}`, true},
		{`{"tabs": {}}`, false},
		{`{"theme": "Dark"}`, false},
	}
	for _, c := range cases {
		if got := load(t, c.text); got.Damaged || got.Backup == c.readable {
			t.Errorf("%s: readable %v, want %v", c.text, !got.Backup, c.readable)
		}
	}
	if s := load(t, `{"clipboardHistoryLimit": -0, "emojiSkinTone": 255}`).Settings; s.ClipboardHistoryLimit != 1 || s.EmojiSkinTone != 5 {
		t.Errorf("%+v", s)
	}
}

// The warning lists the cleaned lists first, then the other fields by name,
// as the Rust app did.
func TestTheWarningNamesFieldsInRustOrder(t *testing.T) {
	loaded := load(t, `{"theme": 1, "tabs": ["x"], "emojiLanguages": 3, "checkForUpdates": "no"}`)
	if !slices.Equal(loaded.dropped, []string{"tabs", "checkForUpdates", "emojiLanguages", "theme"}) {
		t.Errorf("%q", loaded.dropped)
	}
}

func TestNormalizingClampsAndDeduplicates(t *testing.T) {
	s := Default()
	s.ClipboardHistoryLimit = 0
	s.EmojiSkinTone = 9
	s.EmojiLanguages = []EmojiLanguage{"es", "zh", "ms", "zh"}
	s.FileSearchFolders = []string{" ~/A ", "~/A", "  "}
	s.Shortcut = " Alt+Space\n"
	n := s.Normalize()
	if n.ClipboardHistoryLimit != 1 || n.EmojiSkinTone != 5 || n.Shortcut != "Alt+Space" ||
		!slices.Equal(n.EmojiLanguages, []EmojiLanguage{"zh", "ms", "es"}) || !slices.Equal(n.FileSearchFolders, []string{"~/A"}) {
		t.Errorf("%+v", n)
	}
	if s.FileSearchFolders[0] != " ~/A " || s.EmojiLanguages[0] != "es" {
		t.Error("Normalize changed the settings it was given")
	}
	s.ClipboardHistoryLimit = 5000
	if s.Normalize().ClipboardHistoryLimit != ClipboardLimitMax {
		t.Error("limit above the maximum")
	}

	s = Default()
	s.ClockCities = []string{" Tokyo", "tokyo", "", "London", "Paris", "Lima"}
	if got := s.Normalize().ClockCities; !slices.Equal(got, []string{"Tokyo", "London", "Paris"}) {
		t.Errorf("cities %q", got)
	}

	tab := func(c core.Category, shown bool) Tab { return Tab{c, shown, true} }
	s = Default()
	s.Tabs = []Tab{tab(core.CategoryEmoji, false), tab(core.CategoryAll, true), tab(core.CategoryApps, true), tab(core.CategoryEmoji, true)}
	want := []Tab{
		tab(core.CategoryEmoji, false), tab(core.CategoryApps, true), tab(core.CategoryFiles, true),
		tab(core.CategoryClipboard, true), tab(core.CategorySnippets, true), tab(core.CategorySystem, true),
	}
	if got := s.Normalize().Tabs; !slices.Equal(got, want) {
		t.Errorf("tabs %v", got)
	}
	// A tab that is neither shown nor in All would be unreachable.
	s.Tabs = []Tab{{core.CategoryFiles, false, false}}
	if n := s.Normalize(); !n.Tabs[0].InAll || !n.InAll(core.CategoryFiles) || !n.InAll(core.CategoryAll) {
		t.Errorf("%+v", n.Tabs)
	}

	s = Default()
	s.FileSearchExcludedDirs = nil
	for n := range 99 {
		s.FileSearchExcludedDirs = append(s.FileSearchExcludedDirs, fmt.Sprintf("dir%d", n))
	}
	if got := s.Normalize().FileSearchExcludedDirs; len(got) != folderListMax || got[0] != "dir0" {
		t.Errorf("%d dirs", len(got))
	}
}

func TestFindsAFolderThatIsNotAFullPath(t *testing.T) {
	home := filepath.Join(string(filepath.Separator), "home", "me")
	if filepath.VolumeName(t.TempDir()) != "" {
		home = filepath.Join(filepath.VolumeName(t.TempDir())+`\`, "home", "me")
	}
	s := Default()
	s.FileSearchFolders = []string{"~/Notes", "Projects"}
	if got, ok := s.RelativeFolder(home); !ok || got != "Projects" {
		t.Errorf("%q %v", got, ok)
	}
	s.FileSearchFolders = []string{"~", "~/Notes", "relative"}
	if _, ok := s.RelativeFolder(home); !ok {
		t.Error("relative was not found")
	}
	want := []string{home, home + string(filepath.Separator) + "Notes"}
	if got := s.FileFolders(home); !slices.Equal(got, want) {
		t.Errorf("folders %q, want %q", got, want)
	}
}

func TestExpandsTheHomeFolder(t *testing.T) {
	sep := string(filepath.Separator)
	cases := []struct{ path, want string }{
		{"~", "H"},
		{"~/a/../b", "H" + sep + "a/../b"},
		{`~\a`, "H" + sep + "a"},
		{"~user/a", "~user/a"},
		{"/abs", "/abs"},
	}
	for _, c := range cases {
		if got := ExpandHome(c.path, "H"); got != c.want {
			t.Errorf("%q: %q, want %q", c.path, got, c.want)
		}
	}
}

// Keys this version does not model go back to the file unchanged, so the
// Rust app reads them again after a downgrade.
func TestUnknownAndDroppedKeysAreWrittenBack(t *testing.T) {
	text := `{"theme": "dark", "shellCommands": true, "weatherCity": "<Ipoh & co>",
		"futureFlag": {"nested": [1, 2.50, null]}, "showWeather": true}`
	loaded := load(t, text)
	if loaded.Backup {
		t.Fatal(loaded.Warning("/s", true))
	}
	data, err := loaded.Settings.Marshal()
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), `"weatherCity": "<Ipoh & co>"`) || !strings.Contains(string(data), "2.50") {
		t.Errorf("not kept verbatim:\n%s", data)
	}
	var written map[string]any
	if err := json.Unmarshal(data, &written); err != nil {
		t.Fatal(err)
	}
	for _, key := range []string{"shellCommands", "weatherCity", "futureFlag", "showWeather", "shortcut", "tabs"} {
		if _, ok := written[key]; !ok {
			t.Errorf("%s was not written", key)
		}
	}
	again := Load(data, true)
	if again.Backup || !reflect.DeepEqual(again.Settings, loaded.Settings) {
		t.Errorf("a written file does not load back the same:\n%+v\n%+v", again.Settings, loaded.Settings)
	}
	// WithChanges keeps them too, without sharing the map.
	changed, err := loaded.Settings.WithChanges(map[string]json.RawMessage{"theme": json.RawMessage(`"light"`)})
	if err != nil || string(changed.kept["shellCommands"]) != "true" {
		t.Errorf("%v %v", changed.kept, err)
	}
	if !strings.HasPrefix(string(data), "{\n  \"shortcut\": ") {
		t.Errorf("not indented as the Rust app wrote it:\n%s", data)
	}
}

func TestDefaultsLoadBackUnchanged(t *testing.T) {
	data, err := Default().Marshal()
	if err != nil {
		t.Fatal(err)
	}
	if loaded := Load(data, true); loaded.Backup || !reflect.DeepEqual(loaded.Settings, Default()) {
		t.Errorf("%+v", loaded)
	}
}

// A focus timer saved by the Rust app loads as it was, and lengths out of
// range clamp to the Rust app's limits.
func TestFocusTimerSettingsLoadAndClamp(t *testing.T) {
	loaded := load(t, `{"showFocusTimer": true, "focusMinutes": 50, "shortBreakMinutes": 7,
		"longBreakMinutes": 20, "sessionsBeforeLongBreak": 3}`)
	s := loaded.Settings
	if loaded.Backup || !s.ShowFocusTimer || s.FocusMinutes != 50 || s.ShortBreakMinutes != 7 ||
		s.LongBreakMinutes != 20 || s.SessionsBeforeLongBreak != 3 {
		t.Errorf("%+v", s)
	}
	low := Settings{}.Normalize()
	if low.FocusMinutes != 1 || low.ShortBreakMinutes != 1 || low.LongBreakMinutes != 1 || low.SessionsBeforeLongBreak != 1 {
		t.Errorf("%+v", low)
	}
	high := Settings{FocusMinutes: 500, ShortBreakMinutes: 61, LongBreakMinutes: 900, SessionsBeforeLongBreak: 9}.Normalize()
	if high.FocusMinutes != FocusMinutesMax || high.ShortBreakMinutes != BreakMinutesMax ||
		high.LongBreakMinutes != BreakMinutesMax || high.SessionsBeforeLongBreak != SessionsMax {
		t.Errorf("%+v", high)
	}
	// A number too large for the Rust field could not be read there either.
	if big := load(t, `{"focusMinutes": 70000}`); !big.Backup || big.Settings.FocusMinutes != Default().FocusMinutes {
		t.Errorf("%+v", big.Settings)
	}
}
