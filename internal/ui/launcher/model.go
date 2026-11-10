package launcher

import (
	"errors"
	"slices"
	"strings"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// historyLimit is how many past searches Up brings back.
const historyLimit = 50

// Model is the launcher's state and behavior. The view reads it and the
// keymap calls its methods; all of it runs on the main thread.
type Model struct {
	backend Backend

	query    string
	category core.Category
	results  []core.Result
	selected int
	// notice is why the last action failed, until the next edit.
	notice string
	// enableError is why clipboard history did not turn on.
	enableError string
	// warnings are problems found at startup that the user has not
	// dismissed yet, oldest first.
	warnings []string

	// widgets is what the pane showed from the last load. One load runs at
	// a time, as reading the calendars can wait seconds: widgetsLoading is
	// set while it runs, and widgetsAgain asks for one more after it, so
	// changes that come quickly cost two loads at most. widgetsLoaded is
	// set once a load has ended.
	widgets                                     Widgets
	widgetsLoading, widgetsAgain, widgetsLoaded bool
	// shown is set while the panel shows. Up Next loads again as time
	// passes, and as the data changes, only then: each show loads anyway.
	shown bool

	// detailsWanted is the result whose details were asked for last, and
	// detailsStale asks for them again after a search.
	// detailLoads counts loads, so only the latest one shows. details
	// belong to detailsOf.
	detailsWanted core.ID
	detailsStale  bool
	detailLoads   int
	detailsOf     core.ID
	details       Details
	hasDetails    bool

	// history holds queries that ran a result, newest first, only while
	// TinyDash runs: a query can hold private text, so it is never saved.
	// recalled is the entry in the search field, until the user edits it,
	// or -1.
	history  []string
	recalled int

	// running is set while an action runs, so a second one does not start
	// on top of it.
	running bool
	// savingHotkey is set while a hotkey saves; the shortcuts stay paused
	// until it ends, so the saved keys are what the app registers again.
	savingHotkey bool
	// savingFor is the dialog whose save runs.
	savingFor *hotkeyDialog
	// paused is set while the global shortcuts are off for recording.
	paused bool
	// dragPending is set from a press on a drag area until a release. A
	// window manager that moves the panel can take that release; the next
	// one ends it then, which the app takes as no drag.
	dragPending bool

	menu    *menu
	alias   *aliasDialog
	hotkey  *hotkeyDialog
	confirm *confirmation

	input ui.Handle
	list  ui.ListState
	// repeat is whether the last key came as an auto-repeat. A text field
	// takes Enter before any shortcut and tells no repeat, so the view
	// notes it as the key comes.
	repeat bool
	// caretToEnd asks the view to put the caret after a query the launcher
	// put in the field.
	caretToEnd bool
	// aliasField is the alias dialog's field, which takes the focus back
	// after a failed save so the user can fix the alias.
	aliasField ui.Handle
}

type confirmation struct {
	action core.ResultAction
	from   core.ID
}

type aliasDialog struct {
	id     core.ID
	title  string
	text   string
	err    string
	saving bool
}

type hotkeyDialog struct {
	id       core.ID
	title    string
	shortcut string // the hotkey the result has now, or ""
	rec      kit.Recording
	// recorderFocused is whether the recorder had the focus in the last
	// frame, so losing it, as to another app, stops recording.
	recorderFocused bool
}

// New returns a launcher on the All tab with an empty query.
func New(b Backend) *Model {
	m := &Model{backend: b, category: core.CategoryAll, recalled: -1}
	m.search(false)
	return m
}

// Show starts over as the panel shows: an empty query on category, or on
// All for "". Open dialogs and the menu close; a save that still runs ends
// without them.
func (m *Model) Show(category core.Category) {
	if category == "" {
		category = core.CategoryAll
	}
	m.menu, m.alias, m.confirm = nil, nil, nil
	m.closeHotkeyNow()
	m.notice, m.enableError = "", ""
	m.query, m.recalled = "", -1
	m.category = category
	m.warnings = append(m.warnings, m.backend.Warnings()...)
	m.search(false)
	m.shown = true
	m.loadWidgets()
	m.input.Focus()
}

// Hidden stops recording as the panel hides: recording pauses the
// launcher shortcut and every hotkey, which must work again while the
// panel is away. A hide need not blur the view first.
func (m *Model) Hidden() {
	m.shown = false
	m.stopRecording("")
}

// Shown notes that the panel shows, also when the app shows it again as
// it was, without Show, after an action that hid it failed.
func (m *Model) Shown() { m.shown = true }

// Stale searches again after the data changed, keeping the selected
// result, and loads the widgets again while the launcher shows: a setting
// may have changed them. A hidden launcher loads them as it shows, so a
// change while it hides, such as a copy, reads no calendars. Until the
// first load ends, it loads them anyway, so the first show has them.
func (m *Model) Stale() {
	m.search(true)
	if m.shown || !m.widgetsLoaded {
		m.loadWidgets()
	}
}

func (m *Model) loadWidgets() {
	if m.widgetsLoading {
		m.widgetsAgain = true
		return
	}
	m.widgetsLoading = true
	m.backend.Widgets(func(w Widgets) {
		m.widgets, m.widgetsLoading, m.widgetsLoaded = w, false, true
		if m.widgetsAgain {
			m.widgetsAgain = false
			m.loadWidgets()
		}
	})
}

// showsWidgets reports whether the widget pane shows: on All with nothing
// typed, while a widget is on.
func (m *Model) showsWidgets() bool {
	return m.category == core.CategoryAll && m.query == "" && (m.widgets.Clocks || m.widgets.UpNext != nil || m.widgets.Disk != nil)
}

// upNextChanged loads the widgets again once Up Next changes with the
// time, as an event starts or ends, while the launcher shows. It reports
// how long until then, or 0 when it loads now or need not load.
func (m *Model) upNextChanged(now time.Time) time.Duration {
	u := m.widgets.UpNext
	if u == nil || !m.shown || m.widgetsLoading {
		return 0
	}
	if wait := u.ChangesAt.Sub(now); wait > 0 {
		return wait
	}
	m.loadWidgets()
	return 0
}

// warning is the startup warning on screen, or "". An action's failure
// takes its place until the next edit.
func (m *Model) warning() string {
	if m.notice != "" || len(m.warnings) == 0 {
		return ""
	}
	return m.warnings[0]
}

func (m *Model) dismissWarning() {
	if m.warning() != "" {
		m.warnings = m.warnings[1:]
	}
}

// TypeQuery puts text in the search field, as a keyword row asks once the
// engine checked it.
func (m *Model) TypeQuery(text string) {
	m.menu = nil
	m.setQuery(text)
	m.input.Focus()
}

// search shows the results of the query and the tab. keep holds the
// selected result, or its position when it is gone, as after a delete; a
// new query or tab selects the first row.
func (m *Model) search(keep bool) {
	var id core.ID
	if keep && m.selected < len(m.results) {
		id = m.results[m.selected].ID
	}
	m.results = m.backend.Search(m.query, m.category)
	// New results may hold an edited item, as the Rust pane's revision
	// had it: its details load again.
	m.detailsStale = true
	switch i := slices.IndexFunc(m.results, func(r core.Result) bool { return r.ID == id }); {
	case !keep:
		m.selected = 0
	case i >= 0:
		m.selected = i
	default:
		m.selected = max(0, min(m.selected, len(m.results)-1))
	}
	m.scrollToSelected()
}

// rows are the results on screen. While the Clipboard tab says history is
// off it shows none, and keys must not act on hidden ones.
func (m *Model) rows() []core.Result {
	if m.clipboardOff() {
		return nil
	}
	return m.results
}

func (m *Model) selectedRow() (core.Result, bool) {
	rows := m.rows()
	if m.selected < len(rows) {
		return rows[m.selected], true
	}
	return core.Result{}, false
}

func (m *Model) clipboardOff() bool {
	return m.category == core.CategoryClipboard && !m.backend.Settings().ClipboardHistory
}

func (m *Model) scrollToSelected() {
	if m.selected < len(m.rows()) {
		m.list.ScrollIntoView(m.selected)
	}
}

// edited runs after the user changed the search field.
func (m *Model) edited() {
	m.recalled = -1
	m.notice = ""
	m.search(false)
}

func (m *Model) setQuery(q string) {
	m.query = q
	m.caretToEnd = true
	m.edited()
}

func (m *Model) setCategory(category core.Category) {
	m.category = category
	m.notice = ""
	m.search(false)
}

// tabs are All, then the tabs Settings shows. A hidden tab opened on
// purpose shows while it is open, so the view has a name.
func (m *Model) tabs() []core.Category {
	tabs := append([]core.Category{core.CategoryAll}, m.backend.Settings().Tabs...)
	if !slices.Contains(tabs, m.category) {
		tabs = append(tabs, m.category)
	}
	return tabs
}

// moveCategory goes to the next or previous tab shown. With one tab there
// is nowhere to go, and starting it over would drop the error on screen.
func (m *Model) moveCategory(by int) {
	tabs := m.tabs()
	i := slices.Index(tabs, m.category)
	next := tabs[(i+by+len(tabs))%len(tabs)]
	if next != m.category {
		m.setCategory(next)
	}
}

func (m *Model) move(by int) {
	m.selected = max(0, min(m.selected+by, len(m.rows())-1))
	m.scrollToSelected()
}

// up brings back a past search on the first row, else moves up. A held Up
// stops at the first row: only a new press brings back a search.
func (m *Model) up(repeat bool) {
	if !repeat && m.recall() {
		return
	}
	m.move(-1)
}

// recall puts the search before the one in the field, when the field is
// empty or Up filled it, and reports whether it did.
func (m *Model) recall() bool {
	if m.selected != 0 {
		return false
	}
	fromHistory := m.recalled >= 0 && m.recalled < len(m.history) && m.history[m.recalled] == m.query
	if m.query != "" && !fromHistory {
		return false
	}
	next := 0
	if fromHistory {
		next = m.recalled + 1
	}
	if next >= len(m.history) {
		return false
	}
	m.setQuery(m.history[next])
	m.recalled = next
	return true
}

func (m *Model) remember(text string) {
	m.history = slices.DeleteFunc(m.history, func(s string) bool { return s == text })
	m.history = slices.Insert(m.history, 0, text)
	m.history = m.history[:min(len(m.history), historyLimit)]
}

// runSelected runs the selected row's main action, or turns clipboard
// history on from the Clipboard tab's empty view.
func (m *Model) runSelected() {
	if m.clipboardOff() {
		m.enableClipboard()
		return
	}
	m.runRow(m.selected, 0)
}

func (m *Model) runSecondary() { m.runRow(m.selected, 1) }

// runRow runs the action at position of a row, if both are there.
func (m *Model) runRow(row, position int) {
	rows := m.rows()
	if row < len(rows) && position < len(rows[row].Actions) {
		m.runResult(rows[row], position)
	}
}

// runResult runs one of a result's actions. Usage ranking learns only from
// the main action.
func (m *Model) runResult(r core.Result, position int) {
	var from core.ID
	if position == 0 {
		from = r.ID
	}
	// A keyword row only types; the query worth keeping is the one it starts.
	if q := strings.TrimSpace(m.query); q != "" && !isTypeQuery(r.Actions[position].Action) {
		m.remember(q)
	}
	m.start(r.Actions[position], r.Title, from)
}

func isTypeQuery(a core.Action) bool {
	_, ok := a.(core.TypeQuery)
	return ok
}

// start runs an action, or first asks for what it needs: an alias, keys,
// or a confirmation.
func (m *Model) start(ra core.ResultAction, title string, from core.ID) {
	switch a := ra.Action.(type) {
	case core.SetAlias:
		m.alias = &aliasDialog{id: a.ID, title: title, text: a.Alias}
	case core.SetHotkey:
		m.hotkey = &hotkeyDialog{id: a.ID, title: title, shortcut: a.Shortcut}
		m.startRecording()
	default:
		if ra.Confirm != "" {
			m.confirm = &confirmation{action: ra, from: from}
		} else {
			m.perform(ra.Action, from)
		}
	}
}

func (m *Model) perform(a core.Action, from core.ID) {
	if m.running {
		return
	}
	m.running = true
	m.notice = ""
	m.backend.Run(a, from, func(err error) {
		m.running = false
		if err != nil {
			m.notice = err.Error()
		}
	})
}

// deletion is the selected row's action that deletes a history entry,
// which Mod+Backspace runs, and its position.
func (m *Model) deletion() (core.Result, int, bool) {
	r, ok := m.selectedRow()
	if !ok {
		return r, 0, false
	}
	i := slices.IndexFunc(r.Actions, func(ra core.ResultAction) bool { return deletesEntry(ra.Action) })
	return r, i, i >= 0
}

func deletesEntry(a core.Action) bool {
	switch a.(type) {
	case core.DeleteClip, core.DeleteCalculation:
		return true
	}
	return false
}

func (m *Model) canDelete() bool {
	_, _, ok := m.deletion()
	return ok
}

func (m *Model) deleteSelected() {
	if r, i, ok := m.deletion(); ok {
		m.runResult(r, i)
	}
}

func (m *Model) openSettings() { m.start(core.Do("Settings", core.OpenSettings{}), "", "") }

func (m *Model) enableClipboard() {
	m.enableError = ""
	m.backend.EnableClipboardHistory(func(err error) {
		if err != nil {
			m.enableError = err.Error()
		}
	})
}

// escape closes what is on top: a dialog, then the menu, then a startup
// warning; with nothing left it hides the launcher with its query, as the
// Rust app did.
func (m *Model) escape() {
	switch {
	case m.confirm != nil:
		m.cancelConfirm()
	case m.alias != nil:
		m.cancelAlias()
	case m.hotkey != nil && m.hotkey.rec.On():
		m.stopRecording("")
	case m.hotkey != nil:
		m.closeHotkey()
	case m.menu != nil:
		m.closeMenu()
	case m.warning() != "":
		m.dismissWarning()
	default:
		m.backend.Hide()
	}
}

// overlay reports whether a dialog or the menu takes the keyboard.
func (m *Model) overlay() bool {
	return m.menu != nil || m.alias != nil || m.hotkey != nil || m.confirm != nil
}

func (m *Model) focusInput() {
	if !m.overlay() {
		m.input.Focus()
	}
}

func (m *Model) confirmPending() {
	c := m.confirm
	m.confirm = nil
	if c != nil {
		m.perform(c.action.Action, c.from)
	}
	m.focusInput()
}

func (m *Model) cancelConfirm() {
	m.confirm = nil
	m.focusInput()
}

// saveAlias saves the alias typed; empty removes it. The dialog closes once
// it is saved, and shows the reason when it is not.
func (m *Model) saveAlias() {
	d := m.alias
	if d == nil || d.saving {
		return
	}
	if m.running {
		d.err = "Another action is still running. Save again when it ends."
		return
	}
	m.running, d.saving, d.err = true, true, ""
	m.backend.Run(core.SetAlias{ID: d.id, Alias: d.text}, "", func(err error) {
		m.running, d.saving = false, false
		// The launcher may have been shown again meanwhile, with a new start.
		if m.alias != d {
			return
		}
		if err != nil {
			d.err = err.Error()
			m.aliasField.Focus()
			return
		}
		m.alias = nil
		m.focusInput()
	})
}

// cancelAlias closes the alias dialog. A save already sent cannot be taken
// back, so it is not cancelled.
func (m *Model) cancelAlias() {
	if m.alias == nil || m.alias.saving {
		return
	}
	m.alias = nil
	m.focusInput()
}

// errHotkeySaving is why recording does not start while a save runs.
var errHotkeySaving = errors.New("Another hotkey is still saving. Record again when it ends.")

// startRecording pauses the global shortcuts, so pressing one records it.
// If they cannot pause, pressing the current one would open the launcher,
// so recording stops and says why.
func (m *Model) startRecording() {
	d := m.hotkey
	if d == nil {
		return
	}
	// Keys recorded now could not be saved until the running save ends.
	// The dialog's own save keeps its place, so its refusal can show;
	// another dialog's save is the reason given.
	if m.savingHotkey && m.savingFor == d {
		return
	}
	n := d.rec.Start()
	if m.savingHotkey {
		d.rec.Fail(n, errHotkeySaving)
		return
	}
	if m.paused {
		return
	}
	if err := m.backend.PauseShortcuts(true); err != nil {
		d.rec.Fail(n, err)
		return
	}
	m.paused = true
}

// stopRecording ends recording with the keys pressed, or "" for none, and
// saves new keys at once.
func (m *Model) stopRecording(shortcut string) {
	d := m.hotkey
	if d == nil || !d.rec.On() {
		return
	}
	d.rec.Stop()
	if shortcut != "" && shortcut != d.shortcut {
		m.saveHotkey(shortcut)
		return
	}
	m.resumeShortcuts()
}

// saveHotkey saves keys for the result; "" removes its hotkey. The dialog
// closes once they are saved, and shows the reason when they are not.
func (m *Model) saveHotkey(shortcut string) {
	d := m.hotkey
	if d == nil || m.savingHotkey {
		return
	}
	n := d.rec.Save()
	m.savingHotkey, m.savingFor = true, d
	m.backend.Run(core.SetHotkey{ID: d.id, Shortcut: shortcut}, "", func(err error) {
		m.savingHotkey, m.savingFor = false, nil
		if m.hotkey == d {
			if err != nil {
				d.rec.Fail(n, err)
			} else {
				m.hotkey = nil
				m.focusInput()
			}
		}
		m.resumeShortcuts()
	})
}

// closeHotkey closes the hotkey dialog, unless a save runs: it cannot be
// taken back.
func (m *Model) closeHotkey() {
	if m.savingHotkey {
		return
	}
	m.closeHotkeyNow()
	m.focusInput()
}

func (m *Model) closeHotkeyNow() {
	m.hotkey = nil
	m.resumeShortcuts()
}

// resumeShortcuts turns the global shortcuts on again once nothing records
// and no save runs: the app then registers the keys saved last.
func (m *Model) resumeShortcuts() {
	if !m.paused || m.savingHotkey || m.hotkey != nil && m.hotkey.rec.On() {
		return
	}
	m.paused = false
	err := m.backend.PauseShortcuts(false)
	switch {
	case m.hotkey != nil:
		m.hotkey.rec.Resumed(m.hotkey.rec.Last(), err)
	case err != nil:
		m.notice = err.Error()
	}
}
