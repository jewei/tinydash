// Package launcher is the launcher panel's native view and its model: the
// search field, the tabs, the results, the keyboard, the actions menu, and
// the alias, hotkey, and confirmation dialogs. It owns no window: the app
// shows and hides the panel and implements Backend.
package launcher

import (
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/settings"
)

// Backend is what the launcher needs from the app. Every method runs on the
// main thread, and every done function must be called there too (through
// Window.Update), once.
type Backend interface {
	// Search runs for each keystroke and tab change, and again on Stale,
	// so it must answer at once: the engine's snapshot search does.
	Search(query string, category core.Category) []core.Result
	// Run starts an action and calls done with its outcome. Saving an alias
	// or a hotkey can fail on a conflict, and the dialog must stay open with
	// the reason, so the outcome comes back instead of only a log line. from
	// is the result whose main action this is, for usage ranking, or "".
	Run(action core.Action, from core.ID, done func(error))
	// Settings are the settings that change this view. The launcher reads
	// them as it draws, so a change in Settings shows at once.
	Settings() ViewSettings
	// FileIcon is the system icon of a file result, or nil while there is
	// none. Loading one reads the disk, so the app loads and caches it
	// outside the frame.
	FileIcon(path string) *ui.Bitmap
	// PauseShortcuts turns the launcher shortcut and every hotkey off while
	// the hotkey dialog records, so pressing one records it instead of
	// running it, and on again after.
	PauseShortcuts(paused bool) error
	// EnableClipboardHistory turns history on from the Clipboard tab's
	// empty view; done as for Run.
	EnableClipboardHistory(done func(error))
	// Hide hides the panel, as Escape with nothing else to close does.
	Hide()
	// DragStarted says that a press on the empty part of the tab bar or the
	// footer is moving the panel. Where the window manager moves windows
	// itself (X11), the move takes focus from the panel, and the app must
	// not take that blur for the user leaving.
	DragStarted()
	// DragEnded says that the press DragStarted told of was let go of.
	DragEnded()
	// Widgets loads the pane beside an empty All search, each time the
	// launcher shows and after the data changed; done as for Run. Reading
	// the disk can wait on the OS, so the app loads it outside the frame.
	Widgets(done func(Widgets))
	// SaveNote saves the Notepad's note; done as for Run. The launcher
	// sends one save at a time.
	SaveNote(text string, done func(error))
	// Details loads the details of the selected result, which the pane
	// beside a typed search shows; done gets them, or false when the
	// result has none. Only apps, files, folders, clipboard entries, and
	// snippets have them. Reading them can wait on the disk, so the app
	// loads them outside the frame.
	Details(id core.ID, done func(Details, bool))
	// ControlFocus starts, pauses, skips, or resets the focus timer, as
	// one of its FocusActions asks, and returns the timer as it is now,
	// also when it refused, or nil while it is off. It only changes
	// memory, so it answers at once.
	ControlFocus(control FocusControl) (*FocusTimer, error)
	// FocusTimer is the focus timer as it is now, or nil while it is off.
	// It only reads memory, so a pane that shows again has the timer's
	// state at once, not the one from before it hid.
	FocusTimer() *FocusTimer
	// Warnings are the problems found since the last call, such as a
	// hotkey that did not register at startup, each returned once. They
	// come after the launcher was made, so it asks each time it shows.
	Warnings() []string
}

// ViewSettings are the settings that the launcher shows.
type ViewSettings struct {
	// Tabs are the tabs after All that show, in their order.
	Tabs []core.Category
	// ClipboardHistory is off when the user turned history off: the
	// Clipboard tab then offers to turn it on and shows no entries.
	ClipboardHistory bool
	// CurrencyRates makes Refresh also download exchange rates, so its
	// label says so.
	CurrencyRates bool
	// Weather makes Refresh also download the weather: the widget is on
	// and has a city.
	Weather bool
	// Theme is the Appearance setting.
	Theme settings.Theme
}

// Widgets are what the pane beside an empty All search shows. With no
// widget on, there is no pane.
type Widgets struct {
	// Clocks is on while the clocks widget shows: local time, and Cities.
	// Zone is the computer's time zone as the load read it, for the local
	// time; Go's time.Local keeps the zone that TinyDash started in.
	Clocks bool
	Zone   *time.Location
	Cities []City
	// UpNext is nil while Up Next is off, and where TinyDash reads no
	// calendars.
	UpNext *UpNext
	// Disk is nil while the disk widget is off.
	Disk *Disk
	// Weather is nil while the weather widget is off.
	Weather *Weather
	// ClipCards is on while clipboard cards are: the pane shows, with a
	// hint while the clipboard holds nothing a card shows.
	ClipCards bool
	// Card is the card of the clipboard text, or nil.
	Card *ClipCard
	// Focus is nil while the focus timer is off. The launcher fills it
	// from FocusTimer, not from a load, which can take seconds.
	Focus *FocusTimer
	// Note is nil while the Notepad is off.
	Note *Note
}

// Note is the Notepad's saved note.
type Note struct {
	Text string
	// Err is why the note could not be read, or nil.
	Err error
}

// UpNext is the rest of today's events and the reminders due today.
type UpNext struct {
	// Items are the events, then the reminders.
	Items []UpNextItem
	// Problems say why a kind of item cannot show, and how to fix it.
	Problems []string
	// Ask asks the OS for access, while the user has not answered.
	Ask *core.ResultAction
	// ChangesAt is when the list changes as time passes. While the
	// launcher shows, the pane loads again then.
	ChangesAt time.Time
}

// UpNextItem is one event or reminder.
type UpNextItem struct {
	Kind  core.UpNextKind
	Title string
	// Time is when, in words, such as "Now", "10:30–11:00", or "Overdue".
	Time string
	// Source is the calendar or the reminder list.
	Source string
	// Action opens Calendar or Reminders.
	Action core.ResultAction
}

// ClipCard decodes the clipboard text. Exactly one of Color, Time, and
// JSON is set.
type ClipCard struct {
	// ID changes with the text, so a dismissed card comes back only for a
	// new copy.
	ID    string
	Color *ClipColor
	Time  *ClipTime
	JSON  *ClipJSON
	// Rows are a color's or a time's values, each copied by the action of
	// the same index.
	Rows []CardRow
	// Actions copy the content in other forms; the first is the main one.
	Actions []core.ResultAction
}

// CardRow is a labeled value, such as HEX and #2F6F5E.
type CardRow struct{ Label, Value string }

// ClipColor is a copied color.
type ClipColor struct {
	// Hex is the swatch's color, such as #2F6F5E.
	Hex string
	// CopiedAs is RGB or HSL when the copy was not hex, or "".
	CopiedAs         string
	OnWhite, OnBlack Contrast
}

// Contrast is a WCAG contrast ratio, such as 5.9:1, and its grade: AAA, AA,
// AA large, or Fail.
type Contrast struct{ Ratio, Grade string }

// ClipTime is a copied Unix time.
type ClipTime struct {
	Raw          string
	Milliseconds bool
	// Relative is how long ago or ahead it is, such as 3 days ago.
	Relative string
}

// ClipJSON is copied JSON: its shape, and its first lines.
type ClipJSON struct {
	Array               bool
	Count, Depth, Bytes int
	Keys                []JSONKey
	// Minified is the start of the JSON without its spaces.
	Minified string
	Lines    []JSONLine
	// MoreLines is how many lines the full view leaves out.
	MoreLines int
}

// JSONKey is a top-level key with a hint of its value, such as {3}.
type JSONKey struct{ Name, Hint string }

// JSONLine is a laid out line, indented by levels of two spaces.
type JSONLine struct {
	Indent int
	Tokens []JSONToken
}

// JSONToken is a piece of a line, colored by its kind.
type JSONToken struct {
	Kind JSONTokenKind
	Text string
}

// JSONTokenKind is what a token is in JSON.
type JSONTokenKind uint8

const (
	JSONKeyToken JSONTokenKind = iota
	JSONString
	JSONNumber
	// JSONLiteral is true, false, or null.
	JSONLiteral
	JSONPunctuation
)

// City is a city's clock, read as the pane loads. The view keeps its time
// current from the offset; the difference is new by the next load.
type City struct {
	Name string
	// OffsetSeconds is seconds east of UTC.
	OffsetSeconds int
	// Difference is how far it is from local time, such as +8h.
	Difference string
}

// Disk is the disk of the home folder.
type Disk struct {
	Name                  string
	TotalBytes, FreeBytes uint64
	// Err is why the OS did not report the disk, or nil.
	Err error
}

// Weather is the weather card: the city's weather, or what keeps it from
// showing.
type Weather struct {
	State WeatherState
	// City is the city setting, in every state but WeatherNoCity.
	City string
	// The rest is set in WeatherReady, except Message.
	Place string
	// Temperature, High, and Low are whole degrees in the chosen unit.
	Temperature, High, Low int
	// Condition is the weather in words, such as Partly cloudy.
	Condition string
	// Icon names the picture: clearDay, clearNight, partlyDay,
	// partlyNight, cloudy, fog, rain, thunder, or snow.
	Icon string
	// RainChance is today's chance of rain in percent, or nil.
	RainChance *uint8
	// UpdatedAt is when the weather was downloaded, in the computer's
	// time zone as the pane loaded, so it follows a change of the zone.
	UpdatedAt time.Time
	// Offline: the latest download failed, so this is older weather.
	Offline bool
	// Message says why the download failed, in WeatherFailed.
	Message string
}

type WeatherState uint8

const (
	// WeatherNoCity: no city is set.
	WeatherNoCity WeatherState = iota
	WeatherLoading
	WeatherReady
	// WeatherNotFound: no place has the city's name.
	WeatherNotFound
	WeatherFailed
)

// FocusTimer is the focus timer as the pane shows it. The view counts
// down to EndsAt itself; the app loads it again as a phase ends.
type FocusTimer struct {
	// Phase names the phase: Focus, Short break, or Long break.
	Phase string
	// Break is set during a break, which the timer shows in calm colors.
	Break  bool
	Paused bool
	// Done is set once a focus session ended and its break waits.
	Done bool
	// EndsAt is when the running phase ends, or zero while it does not
	// run. Remaining is the time left while it does not run, and Total
	// the length of the phase.
	EndsAt           time.Time
	Remaining, Total time.Duration
	// Session is the session of the cycle, from 1, of Sessions.
	Session, Sessions int
	// Actions are what the user can do now. The first is the main one,
	// which Mod+P runs.
	Actions []FocusAction
}

// FocusAction is one control of the focus timer.
type FocusAction struct {
	Label   string
	Control FocusControl
}

// FocusControl is the app's code for a control of the focus timer. The
// launcher hands it back to ControlFocus as it came.
type FocusControl uint8

// Details are a result's details as the pane shows them.
type Details struct {
	engine.Details
	// Image is the decoded engine.DetailsImage, or nil when it did not
	// decode.
	Image *ui.Bitmap
}
