// Package settings is the settings.json model: its defaults, its tolerant
// load, and its clean-up. It reads and writes no file; the engine does.
//
// The keys are the Rust app's, so either app reads the other's file. Keys
// this version does not model, such as those of dropped features, are kept
// as they were and written back, so going back to the Rust app loses
// nothing.
package settings

import (
	"bytes"
	"encoding/json"
	"maps"
	"path/filepath"
	"slices"
	"strings"

	"github.com/jewei/tinydash/internal/core"
)

const (
	FileName = "settings.json"
	// BackupName is where a file that could not be read in full is kept.
	BackupName        = "settings.invalid.json"
	ClipboardLimitMax = 1000
	// folderListMax bounds each folder list. On Linux each indexed folder
	// uses a watch from a per-user limit.
	folderListMax = 50
	// maxClockCities is the clocks widget's limit (MAX_CLOCK_CITIES in the
	// Rust datetime feature).
	maxClockCities = 3
	maxSkinTone    = 5
	// MaxWeatherCityChars bounds the weather widget's city, in characters
	// (MAX_CITY_CHARS in the Rust weather feature).
	MaxWeatherCityChars = 100
	// The focus timer's limits, in minutes and sessions, as the Rust app
	// clamped them.
	FocusMinutesMax = 120
	BreakMinutesMax = 60
	SessionsMax     = 8
)

type Theme string

const (
	ThemeSystem Theme = "system"
	ThemeLight  Theme = "light"
	ThemeDark   Theme = "dark"
)

// SearchEngine is the engine of the fallback web search.
type SearchEngine string

const (
	Google     SearchEngine = "google"
	DuckDuckGo SearchEngine = "duckDuckGo"
	Bing       SearchEngine = "bing"
	Brave      SearchEngine = "brave"
)

// TemperatureUnit is the unit of the weather widget.
type TemperatureUnit string

const (
	Celsius    TemperatureUnit = "celsius"
	Fahrenheit TemperatureUnit = "fahrenheit"
)

// EmojiLanguage is an extra keyword language. English names and shortcodes
// are always on.
type EmojiLanguage string

// emojiLanguages is in the order the Rust enum declares, which is the order
// Normalize sorts into.
var emojiLanguages = []EmojiLanguage{"zh", "ms", "es"}

// Position is where the user dragged the launcher's top-left corner:
// logical points on macOS, physical pixels on Windows and Linux.
type Position struct {
	X float64 `json:"x"`
	Y float64 `json:"y"`
}

// Tab is one of the launcher's tabs after All.
type Tab struct {
	Category core.Category `json:"category"`
	Shown    bool          `json:"shown"`
	// InAll: its search results and suggestions show in All. Out of All,
	// its pins still head the empty All view (pinned clips never do).
	InAll bool `json:"inAll"`
}

// Settings are the user's preferences. Missing fields take their defaults,
// so older and newer files both load.
type Settings struct {
	// Shortcut toggles the launcher, in Tauri accelerator syntax.
	Shortcut                string `json:"shortcut"`
	Theme                   Theme  `json:"theme"`
	HideOnBlur              bool   `json:"hideOnBlur"`
	LaunchAtLogin           bool   `json:"launchAtLogin"`
	ShowTrayIcon            bool   `json:"showTrayIcon"`
	ClipboardHistoryEnabled bool   `json:"clipboardHistoryEnabled"`
	ClipboardHistoryLimit   uint32 `json:"clipboardHistoryLimit"`
	ClipboardCaptureImages  bool   `json:"clipboardCaptureImages"`
	ClipboardCaptureFiles   bool   `json:"clipboardCaptureFiles"`
	// FileSearchFolders are the folders to index. A leading ~ means the
	// home folder.
	FileSearchFolders []string `json:"fileSearchFolders"`
	// FileSearchExcludedDirs are folder names never indexed, at any depth.
	FileSearchExcludedDirs []string `json:"fileSearchExcludedDirs"`
	// SpotlightFiles: the Files tab also asks the OS's file index for
	// names under the home folder.
	SpotlightFiles bool `json:"spotlightFiles"`
	// EmojiSkinTone is 0 for the default yellow; 1–5 are light to dark.
	EmojiSkinTone        uint8           `json:"emojiSkinTone"`
	EmojiLanguages       []EmojiLanguage `json:"emojiLanguages"`
	CurrencyRatesEnabled bool            `json:"currencyRatesEnabled"`
	CheckForUpdates      bool            `json:"checkForUpdates"`
	SearchEngine         SearchEngine    `json:"searchEngine"`
	// LauncherPosition is nil to center the launcher on the screen with
	// the pointer.
	LauncherPosition *Position `json:"launcherPosition"`
	// Tabs lists every tab after All, in launcher order.
	Tabs       []Tab `json:"tabs"`
	ShowClocks bool  `json:"showClocks"`
	// ClockCities are the places the clocks widget shows, as typed.
	ClockCities []string `json:"clockCities"`
	// ShowUpNext shows the rest of today's events and the reminders due
	// today (macOS). Off by default: turning it on asks the OS for access
	// to them.
	ShowUpNext    bool `json:"showUpNext"`
	ShowDiskSpace bool `json:"showDiskSpace"`
	ShowWeather   bool `json:"showWeather"`
	// WeatherCity is the city of the weather widget, as typed; empty
	// until one is set.
	WeatherCity     string          `json:"weatherCity"`
	TemperatureUnit TemperatureUnit `json:"temperatureUnit"`
	// ShowClipboardCards decodes a copied color, Unix time, or JSON in the
	// widget pane. It is off until the user turns it on: the pane then
	// reads the clipboard each time it shows.
	ShowClipboardCards bool `json:"showClipboardCards"`
	// ShowNotepad shows one scratch note, kept in tinydash.db.
	ShowNotepad       bool   `json:"showNotepad"`
	ShowFocusTimer    bool   `json:"showFocusTimer"`
	FocusMinutes      uint16 `json:"focusMinutes"`
	ShortBreakMinutes uint16 `json:"shortBreakMinutes"`
	LongBreakMinutes  uint16 `json:"longBreakMinutes"`
	// SessionsBeforeLongBreak are the focus sessions in a cycle; the last
	// one earns the long break.
	SessionsBeforeLongBreak uint8 `json:"sessionsBeforeLongBreak"`

	// kept holds the keys this version does not model, as they were.
	kept map[string]json.RawMessage
}

// Default returns the settings of a new user.
func Default() Settings {
	tabs := make([]Tab, 0, len(core.Categories())-1)
	for _, c := range core.Categories()[1:] {
		tabs = append(tabs, Tab{Category: c, Shown: true, InAll: true})
	}
	return Settings{
		Shortcut:               "Control+Shift+Space",
		Theme:                  ThemeSystem,
		HideOnBlur:             true,
		ShowTrayIcon:           defaultShowTrayIcon,
		ClipboardHistoryLimit:  200,
		FileSearchFolders:      []string{"~/Desktop", "~/Documents", "~/Downloads"},
		FileSearchExcludedDirs: []string{"node_modules", "target"},
		EmojiLanguages:         []EmojiLanguage{},
		// Currency rates stay off until the user turns them on: it is the
		// one search feature that downloads.
		CheckForUpdates: true,
		SearchEngine:    Google,
		Tabs:            tabs,
		ShowClocks:      true,
		ClockCities:     []string{},
		ShowDiskSpace:   true,
		TemperatureUnit: Celsius,
		// The focus timer stays off until the user turns it on, as in
		// the Rust app.
		FocusMinutes:            25,
		ShortBreakMinutes:       5,
		LongBreakMinutes:        15,
		SessionsBeforeLongBreak: 4,
	}
}

// Normalize clamps numbers, drops blank or duplicate list entries, and cuts
// lists to their limit. Tabs lose All and gain any tab they lack (such as
// one added in a newer version) at the end, shown and in All. A hidden tab
// stays in All.
func (s Settings) Normalize() Settings {
	s.Shortcut = strings.TrimSpace(s.Shortcut)
	s.ClipboardHistoryLimit = min(max(s.ClipboardHistoryLimit, 1), ClipboardLimitMax)
	s.EmojiSkinTone = min(s.EmojiSkinTone, maxSkinTone)
	s.FocusMinutes = min(max(s.FocusMinutes, 1), FocusMinutesMax)
	s.ShortBreakMinutes = min(max(s.ShortBreakMinutes, 1), BreakMinutesMax)
	s.LongBreakMinutes = min(max(s.LongBreakMinutes, 1), BreakMinutesMax)
	s.SessionsBeforeLongBreak = min(max(s.SessionsBeforeLongBreak, 1), SessionsMax)

	languages := slices.Clone(s.EmojiLanguages)
	slices.SortStableFunc(languages, func(a, b EmojiLanguage) int {
		return slices.Index(emojiLanguages, a) - slices.Index(emojiLanguages, b)
	})
	s.EmojiLanguages = slices.Compact(languages)

	listed := map[core.Category]bool{}
	tabs := []Tab{}
	for _, tab := range s.Tabs {
		if tab.Category != core.CategoryAll && !listed[tab.Category] {
			listed[tab.Category] = true
			tabs = append(tabs, tab)
		}
	}
	for _, c := range core.Categories()[1:] {
		if !listed[c] {
			tabs = append(tabs, Tab{Category: c, Shown: true, InAll: true})
		}
	}
	// A tab neither shown nor in All would leave its results nowhere.
	for i := range tabs {
		tabs[i].InAll = tabs[i].InAll || !tabs[i].Shown
	}
	s.Tabs = tabs

	s.FileSearchFolders = cleanList(s.FileSearchFolders, func(s string) string { return s }, folderListMax)
	s.FileSearchExcludedDirs = cleanList(s.FileSearchExcludedDirs, func(s string) string { return s }, folderListMax)
	s.ClockCities = cleanList(s.ClockCities, strings.ToLower, maxClockCities)
	s.WeatherCity = strings.TrimSpace(s.WeatherCity)
	if city := []rune(s.WeatherCity); len(city) > MaxWeatherCityChars {
		s.WeatherCity = string(city[:MaxWeatherCityChars])
	}
	return s
}

// cleanList trims each entry and keeps the first of each non-blank one, up
// to limit. Entries are the same when key gives the same text.
func cleanList(list []string, key func(string) string, limit int) []string {
	seen := map[string]bool{}
	out := []string{}
	for _, item := range list {
		item = strings.TrimSpace(item)
		if item == "" || seen[key(item)] {
			continue
		}
		seen[key(item)] = true
		out = append(out, item)
	}
	return out[:min(len(out), limit)]
}

// InAll reports whether a category's results and suggestions show in All.
// A category with no tab, which only All itself is after Normalize, does.
func (s Settings) InAll(category core.Category) bool {
	for _, tab := range s.Tabs {
		if tab.Category == category {
			return tab.InAll
		}
	}
	return true
}

// RelativeFolder returns the first folder entry that is not a full path,
// which would never be indexed. A hand-edited file may still hold one.
func (s Settings) RelativeFolder(home string) (string, bool) {
	for _, folder := range s.FileSearchFolders {
		if !filepath.IsAbs(ExpandHome(folder, home)) {
			return folder, true
		}
	}
	return "", false
}

// FileFolders returns the indexed folders as absolute paths. Relative
// entries are left out.
func (s Settings) FileFolders(home string) []string {
	var out []string
	for _, folder := range s.FileSearchFolders {
		if path := ExpandHome(folder, home); filepath.IsAbs(path) {
			out = append(out, path)
		}
	}
	return out
}

// ExpandHome resolves "~" and "~/rest" (or "~\rest") against home; other
// paths stay as they are. The path is not cleaned, as Rust's Path::join
// does not clean it.
func ExpandHome(path, home string) string {
	rest, ok := strings.CutPrefix(path, "~")
	switch {
	case !ok:
		return path
	case rest == "":
		return home
	case rest[0] != '/' && rest[0] != '\\':
		return path
	}
	rest = rest[1:]
	switch {
	case filepath.IsAbs(rest):
		return rest
	case home == "" || strings.HasSuffix(home, string(filepath.Separator)):
		return home + rest
	}
	return home + string(filepath.Separator) + rest
}

// MarshalJSON writes the modeled fields in the Rust app's order, then the
// kept keys by name.
func (s Settings) MarshalJSON() ([]byte, error) {
	type modeled Settings
	known, err := encode(modeled(s))
	if err != nil || len(s.kept) == 0 {
		return known, err
	}
	var out bytes.Buffer
	out.Write(known[:len(known)-1])
	for _, name := range slices.Sorted(maps.Keys(s.kept)) {
		key, err := encode(name)
		if err != nil {
			return nil, err
		}
		out.WriteByte(',')
		out.Write(key)
		out.WriteByte(':')
		out.Write(s.kept[name])
	}
	out.WriteByte('}')
	return out.Bytes(), nil
}

// Marshal returns the settings.json text: two-space indents, as the Rust app
// wrote it.
func (s Settings) Marshal() ([]byte, error) {
	data, err := encode(s)
	if err != nil {
		return nil, err
	}
	var out bytes.Buffer
	if err := json.Indent(&out, data, "", "  "); err != nil {
		return nil, err
	}
	return out.Bytes(), nil
}

// encode is json.Marshal without HTML escapes, which the Rust app never
// wrote.
func encode(v any) ([]byte, error) {
	var out bytes.Buffer
	encoder := json.NewEncoder(&out)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(v); err != nil {
		return nil, err
	}
	return bytes.TrimSuffix(out.Bytes(), []byte("\n")), nil
}
