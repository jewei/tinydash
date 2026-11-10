package settings

import (
	"fmt"
	"math"
	"slices"
	"strconv"
	"strings"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/kit"
)

type section string

const (
	general    section = "General"
	widgets    section = "Widgets"
	clipboard  section = "Clipboard"
	search     section = "Search"
	snippets   section = "Snippets"
	quicklinks section = "Quicklinks"
)

var sections = []section{general, widgets, clipboard, search, snippets, quicklinks}

// modKeys is the app-shortcut modifier as the user knows it, on macOS and
// elsewhere.
var modKeys = map[bool]string{true: "⌘", false: "Ctrl"}

// Model is the Settings window's state and behavior. The view reads it and
// calls its methods; all of it runs on the main thread.
type Model struct {
	backend Backend
	// mac and richClipboard say what this OS offers: Spotlight, and saving
	// images and copied files. Tests set them.
	mac, richClipboard bool

	section section
	// saved is what the backend holds; queue holds the changes on their
	// way, the first one sent.
	saved   settings.Settings
	queue   []pending
	sending bool
	// err is why the last change failed, unless it says so in its own
	// place (saveOwn), or another error for the page.
	err string

	// rec is the launcher shortcut's recorder.
	rec kit.Recording
	// steps are the pauses, saves, and resumes of recording, in order;
	// the first one runs.
	steps    []func(next func())
	recorder ui.Handle
	// recorderFocused is whether the recorder had the focus in the last
	// frame: losing it stops recording.
	recorderFocused bool

	folders, skipped, cities listEditor
	limit                    numberField
	// The focus timer's lengths.
	focusMinutes, shortBreak, longBreak, sessions numberField
	// moved is the tab whose row a move button moved, for the focus to
	// follow it.
	moved *tabMove

	hidden, aliases, hotkeys personalList

	confirmClear bool
	clearButton  ui.Handle

	library []LibraryItem
	editors map[bool]*libraryEditor
}

type pending struct {
	change func(*settings.Settings)
	done   func(error)
	// own is set when done shows a refusal in its own place, so the page
	// banner does not show it again.
	own bool
}

type tabMove struct {
	category core.Category
	up       bool
}

// New returns Settings on General, with the saved settings.
func New(b Backend) *Model {
	m := &Model{backend: b, mac: mac, richClipboard: richClipboard, section: general}
	m.editors = map[bool]*libraryEditor{false: newEditor(m, false), true: newEditor(m, true)}
	m.saved = b.Settings()
	m.reload()
	return m
}

// Stale reads the saved settings, the library, and the hidden results
// again after they changed elsewhere, as in the launcher. A change on its
// way brings the saved settings back with it.
func (m *Model) Stale() {
	if !m.sending {
		m.saved = m.backend.Settings()
	}
	m.reload()
}

// Close stops recording as the window closes, so the shortcuts turn on
// again.
func (m *Model) Close() { m.stopRecording("") }

func (m *Model) reload() {
	m.hidden.items = m.backend.Hidden()
	m.aliases.items = m.backend.Aliases()
	m.hotkeys.items = m.backend.Hotkeys()
	m.library = m.backend.Library()
	for _, ed := range m.editors {
		ed.listChanged()
	}
}

// values are the saved settings with the changes on their way on top.
func (m *Model) values() settings.Settings {
	v := m.saved
	for _, p := range m.queue {
		p.change(&v)
	}
	return v
}

// save sends a change after those before it, applied to the settings
// saved by then. done, if set, gets its outcome.
func (m *Model) save(change func(*settings.Settings), done func(error)) {
	m.queue = append(m.queue, pending{change: change, done: done})
	m.send()
}

// saveOwn is save for a change whose done shows a refusal itself.
func (m *Model) saveOwn(change func(*settings.Settings), done func(error)) {
	m.queue = append(m.queue, pending{change: change, done: done, own: true})
	m.send()
}

func (m *Model) send() {
	if m.sending || len(m.queue) == 0 {
		return
	}
	m.sending = true
	p := m.queue[0]
	m.backend.UpdateSettings(p.change, func(saved settings.Settings, err error) {
		m.sending = false
		m.queue = m.queue[1:]
		switch {
		case err == nil:
			m.saved, m.err = saved, ""
		case !p.own:
			m.err = err.Error()
		}
		if p.done != nil {
			p.done(err)
		}
		m.send()
	})
}

func (m *Model) open(s section) {
	m.section = s
	if s == search {
		m.hidden.items = m.backend.Hidden()
		m.aliases.items = m.backend.Aliases()
		m.hotkeys.items = m.backend.Hotkeys()
	}
}

// startRecording pauses the shortcuts and records. If the shortcut cannot
// pause, pressing it would show the launcher instead of being recorded, so
// recording stops and says why.
func (m *Model) startRecording() {
	n := m.rec.Start()
	m.recorder.Focus()
	m.then(func(next func()) {
		m.backend.PauseShortcuts(true, func(err error) {
			m.rec.Fail(n, err)
			next()
		})
	})
}

// stopRecording ends recording with the keys pressed, or "" for none,
// saves new keys, and then turns the shortcuts on again: the saved ones,
// new or old.
func (m *Model) stopRecording(shortcut string) {
	if !m.rec.On() {
		return
	}
	m.rec.Stop()
	n := m.rec.Last()
	changed := shortcut != "" && shortcut != m.values().Shortcut
	m.then(func(next func()) {
		resume := func(saveErr error) {
			m.rec.Fail(n, saveErr)
			m.backend.PauseShortcuts(false, func(err error) {
				m.rec.Resumed(n, err)
				next()
			})
		}
		if changed {
			m.saveOwn(func(s *settings.Settings) { s.Shortcut = shortcut }, resume)
		} else {
			resume(nil)
		}
	})
}

// then runs a step of recording after the steps before it, as pausing,
// saving, and resuming must not overtake each other: a resume that came
// after a newer pause would turn the shortcut on while recording. work
// calls next when it is done.
func (m *Model) then(work func(next func())) {
	m.steps = append(m.steps, work)
	if len(m.steps) == 1 {
		m.runStep()
	}
}

func (m *Model) runStep() {
	if len(m.steps) == 0 {
		return
	}
	m.steps[0](func() {
		m.steps = m.steps[1:]
		m.runStep()
	})
}

// moveTab moves a tab one place up or down, in the saved order.
func (m *Model) moveTab(category core.Category, up bool) {
	m.moved = &tabMove{category, up}
	m.save(func(s *settings.Settings) {
		tabs := slices.Clone(s.Tabs)
		from := slices.IndexFunc(tabs, func(t settings.Tab) bool { return t.Category == category })
		to := from + 1
		if up {
			to = from - 1
		}
		if from < 0 || to < 0 || to >= len(tabs) {
			return
		}
		tabs[from], tabs[to] = tabs[to], tabs[from]
		s.Tabs = tabs
	}, nil)
}

// setTab turns a tab's Show or In All switch. A tab is never both hidden
// and out of All, as Normalize has it, so turning one off while the other
// is off turns the other on; the form shows it at once.
func (m *Model) setTab(category core.Category, shown bool, on bool) {
	m.save(func(s *settings.Settings) {
		tabs := slices.Clone(s.Tabs)
		for i, t := range tabs {
			if t.Category != category {
				continue
			}
			if shown {
				t.Shown = on
			} else {
				t.InAll = on
			}
			if !t.Shown && !t.InAll {
				t.Shown, t.InAll = !shown, shown
			}
			tabs[i] = t
		}
		s.Tabs = tabs
	}, nil)
}

// personalList is a Search list of results the user changed: the hidden
// ones, the aliases, or the hotkeys, each with a button that undoes it.
type personalList struct {
	items []engine.Listed
	// refocus asks the next frame to put the focus at place focus, after
	// a removal.
	refocus bool
	focus   int
	err     string
}

// remove undoes the change of the item at index through the backend, then
// reads the list again and puts the focus on the row now in its place.
func (l *personalList) remove(id core.ID, index int, undo func(core.ID, func(error)), reload func() []engine.Listed) {
	l.err = ""
	undo(id, func(err error) {
		if err != nil {
			l.err = err.Error()
			return
		}
		l.items = reload()
		l.focus, l.refocus = index, true
	})
}

func (m *Model) clearHistory() {
	m.closeClear()
	m.backend.ClearClipboard(func(err error) {
		if err != nil {
			m.err = err.Error()
		}
	})
}

func (m *Model) closeClear() {
	m.confirmClear = false
	m.clearButton.Focus()
}

// listEditor is a list of text, such as folders, with Add and Remove.
type listEditor struct {
	draft  string
	notice string
	field  ui.Handle
}

// add adds the draft to the list that get and set read and write in the
// saved settings, unless it is there. The text stays until the save
// works, so a refused entry can be fixed; the reason shows next to it.
func (m *Model) addTo(l *listEditor, items []string, limit int, ignoreCase bool, get func(*settings.Settings) *[]string) {
	value := strings.TrimSpace(l.draft)
	if len(items) >= limit || value == "" {
		return
	}
	same := func(item string) bool {
		if ignoreCase {
			return strings.EqualFold(item, value)
		}
		return item == value
	}
	if slices.ContainsFunc(items, same) {
		l.notice = fmt.Sprintf("“%s” is already in the list.", value)
		return
	}
	l.notice = ""
	m.save(func(s *settings.Settings) {
		list := get(s)
		if !slices.ContainsFunc(*list, same) {
			*list = append(slices.Clip(*list), value)
		}
	}, func(err error) {
		switch {
		case err != nil:
			l.notice = err.Error()
		case strings.TrimSpace(l.draft) == value:
			l.draft = ""
		}
	})
}

// removeFrom removes item and gives the focus to the field, so it never
// falls back to the window.
func (m *Model) removeFrom(l *listEditor, item string, get func(*settings.Settings) *[]string) {
	l.notice = ""
	m.save(func(s *settings.Settings) {
		list := get(s)
		*list = slices.DeleteFunc(slices.Clone(*list), func(other string) bool { return other == item })
	}, func(err error) {
		if err != nil {
			l.notice = err.Error()
		}
	})
	l.field.Focus()
}

// numberField is a whole number that saves when the field loses the focus
// or Enter is pressed. It shows the saved value while it is not edited.
type numberField struct {
	draft   string
	editing bool
}

// commit saves the draft, rounded and clamped. An empty or invalid draft
// restores the saved value instead of saving 0.
func (m *Model) commit(f *numberField, value, lo, hi int, set func(*settings.Settings, int)) {
	f.editing = false
	n, err := strconv.ParseFloat(strings.TrimSpace(f.draft), 64)
	if err != nil || math.IsNaN(n) || math.IsInf(n, 0) {
		return
	}
	clamped := int(min(max(math.Round(n), float64(lo)), float64(hi)))
	if clamped != value {
		m.save(func(s *settings.Settings) { set(s, clamped) }, nil)
	}
}
