// Package launcher is the launcher panel's native view and its model: the
// search field, the tabs, the results, the keyboard, the actions menu, and
// the alias, hotkey, and confirmation dialogs. It owns no window: the app
// shows and hides the panel and implements Backend.
package launcher

import (
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
	// Widgets loads the pane beside an empty All search, each time the
	// launcher shows and after the data changed; done as for Run. Reading
	// the disk can wait on the OS, so the app loads it outside the frame.
	Widgets(done func(Widgets))
	// Details loads the details of the selected result, which the pane
	// beside a typed search shows; done gets them, or false when the
	// result has none. Only apps, files, folders, clipboard entries, and
	// snippets have them. Reading them can wait on the disk, so the app
	// loads them outside the frame.
	Details(id core.ID, done func(Details, bool))
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
	// Theme is the Appearance setting.
	Theme settings.Theme
}

// Widgets are what the pane beside an empty All search shows. With no
// widget on, there is no pane.
type Widgets struct {
	// Clocks is on while the clocks widget shows: local time, and Cities.
	Clocks bool
	Cities []City
	// Disk is nil while the disk widget is off.
	Disk *Disk
}

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

// Details are a result's details as the pane shows them.
type Details struct {
	engine.Details
	// Image is the decoded engine.DetailsImage, or nil when it did not
	// decode.
	Image *ui.Bitmap
}
