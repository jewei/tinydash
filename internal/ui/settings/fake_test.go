package settings

import (
	"errors"
	"slices"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/settings"
)

// fake is a Backend that keeps settings, a library, and hidden results as
// the app does, and records what Settings asks.
type fake struct {
	saved settings.Settings
	// refuse, if set, is the error of a change that gives these settings.
	refuse func(settings.Settings) error
	// hold keeps replies waiting, for a test to send them later with
	// release.
	hold    bool
	waiting []func()

	updates  []settings.Settings
	pauses   []bool
	pauseErr func(paused bool) error

	library []LibraryItem
	nextID  int64
	saves   []LibraryItem
	deletes []int64
	// saveErr, if set, is the error of a save of this item.
	saveErr func(LibraryItem) error

	hidden  []engine.Listed
	unhides []core.ID
	clears  int
}

func newFake() *fake { return &fake{saved: settings.Default(), nextID: 100} }

// reply sends done now, or later when replies are held.
func (f *fake) reply(done func()) {
	if f.hold {
		f.waiting = append(f.waiting, done)
		return
	}
	done()
}

// release sends the oldest held reply.
func (f *fake) release(t *testing.T, tt *ui.Tester) {
	t.Helper()
	if len(f.waiting) == 0 {
		t.Fatal("no reply waits")
	}
	done := f.waiting[0]
	f.waiting = f.waiting[1:]
	done()
	tt.Frame()
}

func (f *fake) Settings() settings.Settings { return f.saved }

func (f *fake) UpdateSettings(change func(*settings.Settings), done func(settings.Settings, error)) {
	next := f.saved
	change(&next)
	next = next.Normalize()
	f.updates = append(f.updates, next)
	f.reply(func() {
		if f.refuse != nil {
			if err := f.refuse(next); err != nil {
				done(f.saved, err)
				return
			}
		}
		f.saved = next
		done(next, nil)
	})
}

func (f *fake) PauseShortcuts(paused bool) error {
	f.pauses = append(f.pauses, paused)
	if f.pauseErr != nil {
		return f.pauseErr(paused)
	}
	return nil
}

func (f *fake) Library() []LibraryItem { return slices.Clone(f.library) }

func (f *fake) SaveLibraryItem(item LibraryItem, done func(LibraryItem, error)) {
	f.saves = append(f.saves, item)
	f.reply(func() {
		if f.saveErr != nil {
			if err := f.saveErr(item); err != nil {
				done(LibraryItem{}, err)
				return
			}
		}
		if item.ID == 0 {
			f.nextID++
			item.ID = f.nextID
		}
		f.library = append(slices.DeleteFunc(f.library, func(it LibraryItem) bool { return it.ID == item.ID }), item)
		done(item, nil)
	})
}

func (f *fake) DeleteLibraryItem(id int64, done func(error)) {
	f.deletes = append(f.deletes, id)
	f.reply(func() {
		f.library = slices.DeleteFunc(f.library, func(it LibraryItem) bool { return it.ID == id })
		done(nil)
	})
}

func (f *fake) Hidden() []engine.Listed { return slices.Clone(f.hidden) }

func (f *fake) Unhide(id core.ID, done func(error)) {
	f.unhides = append(f.unhides, id)
	f.hidden = slices.DeleteFunc(f.hidden, func(r engine.Listed) bool { return r.ID == id })
	done(nil)
}

func (f *fake) ClearClipboard(done func(error)) {
	f.clears++
	done(nil)
}

// last is the settings the last change sent would save.
func (f *fake) last(t *testing.T) settings.Settings {
	t.Helper()
	if len(f.updates) == 0 {
		t.Fatal("no change was sent")
	}
	return f.updates[len(f.updates)-1]
}

func (f *fake) wantUpdates(t *testing.T, n int) {
	t.Helper()
	if len(f.updates) != n {
		t.Fatalf("%d changes sent, want %d", len(f.updates), n)
	}
}

var errRefused = errors.New("Could not save settings.")

// open starts Settings over f in a tester, on section.
func open(t *testing.T, f *fake, s section) (*Model, *ui.Tester) {
	t.Helper()
	m := New(f)
	m.mac, m.richClipboard = false, true
	tt := ui.NewTester(m.View, 1000, 2400)
	if s != general {
		click(t, tt, string(s))
	}
	return m, tt
}

func click(t *testing.T, tt *ui.Tester, name string) {
	t.Helper()
	if err := tt.Click(name); err != nil {
		t.Fatalf("click %q: %v; texts %q", name, err, tt.Texts())
	}
}

// press clicks the element with that role and name, where other text has
// the same words.
func press(t *testing.T, tt *ui.Tester, role ui.Role, name string) {
	t.Helper()
	info, ok := tt.AccessByRole(role, name)
	if !ok {
		t.Fatalf("no %v %q; texts %q", role, name, tt.Texts())
	}
	tt.ClickAt(info.Bounds.X+info.Bounds.W/2, info.Bounds.Y+info.Bounds.H/2)
}

func flip(t *testing.T, tt *ui.Tester, name string) { t.Helper(); press(t, tt, ui.RoleSwitch, name) }

func checked(t *testing.T, tt *ui.Tester, name string) bool {
	t.Helper()
	info, ok := tt.AccessByRole(ui.RoleSwitch, name)
	if !ok {
		t.Fatalf("no switch %q", name)
	}
	return info.Checked
}

// typeInto replaces the text of the field with that name, then presses
// Enter when enter is set.
func typeInto(t *testing.T, tt *ui.Tester, name, text string, enter bool) {
	t.Helper()
	press(t, tt, ui.RoleTextField, name)
	tt.Key(ui.Cmd, ui.KeyA)
	tt.Key(0, ui.KeyBackspace)
	tt.Type(text)
	if enter {
		tt.Key(0, ui.KeyEnter)
	}
}
