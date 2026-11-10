package settings

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"maps"
	"math"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
)

// Loaded is what Load made of a settings file, and what the engine must do
// with the file before it saves over it.
type Loaded struct {
	Settings Settings
	// Backup: keep the file as BackupName before any save, because a save
	// would lose values this version could not read. Damaged says how.
	Backup bool
	// Damaged: nothing in the file could be read, so the file is moved
	// aside and defaults are in use. Otherwise a backup is a copy.
	Damaged bool
	// reason is the parse error of a damaged file.
	reason string
	// dropped names the fields that lost a value, in the Rust app's order.
	dropped []string
}

// Load reads the content of settings.json; present is false when there is
// no file. A damaged file gives defaults, so a bad edit never stops the
// launcher. A value this version cannot read takes its default; a list
// loses only its unreadable entries.
//
// The Rust app read a JSON array as settings too ("[]" gave defaults); here
// anything but an object is damaged, so such a file is kept aside.
func Load(data []byte, present bool) Loaded {
	if !present {
		return Loaded{Settings: Default()}
	}
	damaged := func(reason string) Loaded {
		return Loaded{Settings: Default(), Backup: true, Damaged: true, reason: reason}
	}
	if !utf8.Valid(data) {
		return damaged("stream did not contain valid UTF-8")
	}
	var file map[string]json.RawMessage
	if err := json.Unmarshal(data, &file); err != nil {
		var syntax *json.SyntaxError
		if errors.As(err, &syntax) {
			return damaged(fmt.Sprintf("%s at %s", syntax, position(data, syntax.Offset)))
		}
		return damaged("the file is not a JSON object")
	}
	if file == nil {
		return damaged("the file is not a JSON object")
	}
	values := map[string]any{}
	for name, raw := range file {
		values[name] = decodeValue(raw)
	}

	settings := Default()
	var dropped []string
	for _, list := range []struct {
		name string
		read func(any) bool
	}{
		{"tabs", func(v any) bool { _, ok := readTab(v); return ok }},
		{"emojiLanguages", func(v any) bool { _, ok := readEmojiLanguage(v); return ok }},
	} {
		entries, ok := values[list.name].([]any)
		if !ok {
			continue
		}
		readable := slices.DeleteFunc(slices.Clone(entries), func(v any) bool { return !list.read(v) })
		if len(readable) != len(entries) {
			values[list.name] = readable
			dropped = append(dropped, list.name)
		}
	}
	for _, name := range slices.Sorted(maps.Keys(file)) {
		read, modeled := fields[name]
		switch {
		case !modeled:
			if settings.kept == nil {
				settings.kept = map[string]json.RawMessage{}
			}
			var compact bytes.Buffer
			if err := json.Compact(&compact, file[name]); err != nil {
				return damaged(err.Error())
			}
			settings.kept[name] = compact.Bytes()
		case !read(&settings, values[name]):
			dropped = append(dropped, name)
		}
	}
	return Loaded{Settings: settings.Normalize(), Backup: len(dropped) > 0, dropped: dropped}
}

// position names the byte before offset, where Go's parser stopped, as
// serde_json words a place: line from 1, and column from 1 within it.
func position(data []byte, offset int64) string {
	at := min(int(offset), len(data)) - 1
	before := data[:max(at, 0)]
	line := bytes.Count(before, []byte("\n")) + 1
	column := at - bytes.LastIndexByte(before, '\n')
	return fmt.Sprintf("line %d column %d", line, column)
}

// Warning is the message for the user, or "" when the load needs none. dir
// is the settings folder; backedUp says whether the engine kept the file as
// BackupName.
func (l Loaded) Warning(dir string, backedUp bool) string {
	backup := filepath.Join(dir, BackupName)
	switch {
	case l.Damaged:
		note := " Changes will not be saved. Fix or remove the file, then reopen TinyDash."
		if backedUp {
			note = fmt.Sprintf(" The old file is at %s.", backup)
		}
		return fmt.Sprintf("Settings were invalid (%s), so defaults are in use.%s", l.reason, note)
	case len(l.dropped) > 0:
		note := ""
		if backedUp {
			note = fmt.Sprintf(" The old file is at %s.", backup)
		}
		return fmt.Sprintf("Some values under these names in settings.json could not be read: %s. "+
			"TinyDash skipped only those values and uses defaults for them.%s", strings.Join(l.dropped, ", "), note)
	}
	return ""
}

// Replaceable reports whether saves may write over the file. A damaged file
// that could not be moved aside holds the only copy of the user's settings.
func (l Loaded) Replaceable(backedUp bool) bool { return !l.Damaged || backedUp }

// ReadFailedWarning is the message when the file exists but could not be
// read, such as while another program locks it. Defaults are in use then,
// and the file must not be replaced.
func ReadFailedWarning(err error) string {
	return fmt.Sprintf("Could not read settings (%v), so defaults are in use and changes will not be saved. Quit and reopen TinyDash to try again.", err)
}

// WithChanges returns the settings with changes, camelCase fields as the
// settings window sends them, on top. Each window sends only what it
// changed, so two windows that save at once do not undo each other.
func (s Settings) WithChanges(changes map[string]json.RawMessage) (Settings, error) {
	s.kept = maps.Clone(s.kept)
	for _, name := range slices.Sorted(maps.Keys(changes)) {
		read, modeled := fields[name]
		if !modeled {
			return Settings{}, Error(fmt.Sprintf("Unknown setting “%s”.", name))
		}
		if !read(&s, decodeValue(changes[name])) {
			return Settings{}, Error(fmt.Sprintf("Invalid value for the setting “%s”: %s", name, changes[name]))
		}
	}
	return s, nil
}

// Error is a message for the user, written as full sentences.
type Error string

func (e Error) Error() string { return string(e) }

// decodeValue parses one JSON value with its numbers as written. JSON null
// is nil, which only launcherPosition reads; invalid JSON is notJSON, which
// no field reads.
func decodeValue(raw json.RawMessage) any {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var v any
	if decoder.Decode(&v) != nil || decoder.More() {
		return notJSON{}
	}
	return v
}

type notJSON struct{}

// fields reads each modeled key as serde read it into the Rust struct: the
// type must match exactly, and a number must fit without rounding.
var fields = map[string]func(*Settings, any) bool{
	"shortcut":                field(func(s *Settings) *string { return &s.Shortcut }, readString),
	"theme":                   field(func(s *Settings) *Theme { return &s.Theme }, readEnum(ThemeSystem, ThemeLight, ThemeDark)),
	"hideOnBlur":              field(func(s *Settings) *bool { return &s.HideOnBlur }, readBool),
	"launchAtLogin":           field(func(s *Settings) *bool { return &s.LaunchAtLogin }, readBool),
	"showTrayIcon":            field(func(s *Settings) *bool { return &s.ShowTrayIcon }, readBool),
	"clipboardHistoryEnabled": field(func(s *Settings) *bool { return &s.ClipboardHistoryEnabled }, readBool),
	"clipboardHistoryLimit":   field(func(s *Settings) *uint32 { return &s.ClipboardHistoryLimit }, readUint[uint32]),
	"clipboardCaptureImages":  field(func(s *Settings) *bool { return &s.ClipboardCaptureImages }, readBool),
	"clipboardCaptureFiles":   field(func(s *Settings) *bool { return &s.ClipboardCaptureFiles }, readBool),
	"fileSearchFolders":       field(func(s *Settings) *[]string { return &s.FileSearchFolders }, readList(readString)),
	"fileSearchExcludedDirs":  field(func(s *Settings) *[]string { return &s.FileSearchExcludedDirs }, readList(readString)),
	"spotlightFiles":          field(func(s *Settings) *bool { return &s.SpotlightFiles }, readBool),
	"emojiSkinTone":           field(func(s *Settings) *uint8 { return &s.EmojiSkinTone }, readUint[uint8]),
	"emojiLanguages":          field(func(s *Settings) *[]EmojiLanguage { return &s.EmojiLanguages }, readList(readEmojiLanguage)),
	"currencyRatesEnabled":    field(func(s *Settings) *bool { return &s.CurrencyRatesEnabled }, readBool),
	"checkForUpdates":         field(func(s *Settings) *bool { return &s.CheckForUpdates }, readBool),
	"searchEngine":            field(func(s *Settings) *SearchEngine { return &s.SearchEngine }, readEnum(Google, DuckDuckGo, Bing, Brave)),
	"launcherPosition":        field(func(s *Settings) **Position { return &s.LauncherPosition }, readPosition),
	"tabs":                    field(func(s *Settings) *[]Tab { return &s.Tabs }, readList(readTab)),
	"showClocks":              field(func(s *Settings) *bool { return &s.ShowClocks }, readBool),
	"clockCities":             field(func(s *Settings) *[]string { return &s.ClockCities }, readList(readString)),
	"showUpNext":              field(func(s *Settings) *bool { return &s.ShowUpNext }, readBool),
	"showDiskSpace":           field(func(s *Settings) *bool { return &s.ShowDiskSpace }, readBool),
	"showWeather":             field(func(s *Settings) *bool { return &s.ShowWeather }, readBool),
	"weatherCity":             field(func(s *Settings) *string { return &s.WeatherCity }, readString),
	"temperatureUnit":         field(func(s *Settings) *TemperatureUnit { return &s.TemperatureUnit }, readEnum(Celsius, Fahrenheit)),
	"showClipboardCards":      field(func(s *Settings) *bool { return &s.ShowClipboardCards }, readBool),
	"showNotepad":             field(func(s *Settings) *bool { return &s.ShowNotepad }, readBool),
	"showFocusTimer":          field(func(s *Settings) *bool { return &s.ShowFocusTimer }, readBool),
	"focusMinutes":            field(func(s *Settings) *uint16 { return &s.FocusMinutes }, readUint[uint16]),
	"shortBreakMinutes":       field(func(s *Settings) *uint16 { return &s.ShortBreakMinutes }, readUint[uint16]),
	"longBreakMinutes":        field(func(s *Settings) *uint16 { return &s.LongBreakMinutes }, readUint[uint16]),
	"sessionsBeforeLongBreak": field(func(s *Settings) *uint8 { return &s.SessionsBeforeLongBreak }, readUint[uint8]),
}

func field[T any](at func(*Settings) *T, read func(any) (T, bool)) func(*Settings, any) bool {
	return func(s *Settings, v any) bool {
		value, ok := read(v)
		if ok {
			*at(s) = value
		}
		return ok
	}
}

func readString(v any) (string, bool) { s, ok := v.(string); return s, ok }

func readBool(v any) (bool, bool) { b, ok := v.(bool); return b, ok }

// readUint takes a whole number in range, as serde does: "1.0" and "1e2"
// are floats, and "-0" is zero.
func readUint[T uint8 | uint16 | uint32](v any) (T, bool) {
	n, ok := v.(json.Number)
	if !ok {
		return 0, false
	}
	limit := uint64(^T(0))
	if u, err := strconv.ParseUint(string(n), 10, 64); err == nil {
		return T(u), u <= limit
	}
	i, err := strconv.ParseInt(string(n), 10, 64)
	return 0, err == nil && i == 0
}

func readFloat(v any) (float64, bool) {
	n, ok := v.(json.Number)
	if !ok {
		return 0, false
	}
	f, err := strconv.ParseFloat(string(n), 64)
	return f, err == nil && !math.IsInf(f, 0)
}

func readEnum[T ~string](values ...T) func(any) (T, bool) {
	return func(v any) (T, bool) {
		s, ok := v.(string)
		if !ok || !slices.Contains(values, T(s)) {
			return "", false
		}
		return T(s), true
	}
}

var readEmojiLanguage = readEnum(emojiLanguages...)

var readCategory = readEnum(core.Categories()...)

func readList[T any](read func(any) (T, bool)) func(any) ([]T, bool) {
	return func(v any) ([]T, bool) {
		entries, ok := v.([]any)
		if !ok {
			return nil, false
		}
		out := make([]T, 0, len(entries))
		for _, entry := range entries {
			value, ok := read(entry)
			if !ok {
				return nil, false
			}
			out = append(out, value)
		}
		return out, true
	}
}

// readTab needs category and shown. inAll is missing in files from before
// the switch, which meant yes. Other keys are ignored, as serde ignores them.
func readTab(v any) (Tab, bool) {
	object, ok := v.(map[string]any)
	if !ok {
		return Tab{}, false
	}
	category, ok := readCategory(object["category"])
	if !ok {
		return Tab{}, false
	}
	shown, ok := readBool(object["shown"])
	if !ok {
		return Tab{}, false
	}
	inAll := true
	if value, present := object["inAll"]; present {
		if inAll, ok = readBool(value); !ok {
			return Tab{}, false
		}
	}
	return Tab{Category: category, Shown: shown, InAll: inAll}, true
}

// readPosition takes null, which centers the launcher, or both coordinates.
func readPosition(v any) (*Position, bool) {
	if v == nil {
		return nil, true
	}
	object, ok := v.(map[string]any)
	if !ok {
		return nil, false
	}
	x, okX := readFloat(object["x"])
	y, okY := readFloat(object["y"])
	if !okX || !okY {
		return nil, false
	}
	return &Position{X: x, Y: y}, true
}
