package app

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/egoist/mygo"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/settings"
)

// The Rust app's login item starts TinyDash with --background, and its
// users may run tinydash --settings or --mode from a desktop shortcut.
func TestTheRustAppsCommandLineStillWorks(t *testing.T) {
	for _, c := range []struct {
		args []string
		want Options
	}{
		{nil, Options{}},
		{[]string{"--background"}, Options{Launch: Launch{Background: true}}},
		{[]string{"--hidden"}, Options{Launch: Launch{Background: true}}},
		{[]string{"--settings"}, Options{Launch: Launch{Settings: true}}},
		{[]string{"--mode", "clipboard"}, Options{Launch: Launch{Category: core.CategoryClipboard}}},
		{[]string{"--data-dir", "scratch", "--background"}, Options{DataDir: "scratch", Launch: Launch{Background: true}}},
	} {
		got, err := ParseArgs(c.args)
		if err != nil || got != c.want {
			t.Errorf("%q: %+v, %v; want %+v", c.args, got, err, c.want)
		}
	}
	for _, args := range [][]string{
		{"--mode", "nope"},
		{"--mode"},
		{"--settings", "--background"},
		{"--hidden", "--mode", "apps"},
		{"stray"},
		{"--unknown"},
	} {
		if _, err := ParseArgs(args); err == nil || errors.Is(err, ErrHelp) || !strings.Contains(err.Error(), "Usage: tinydash") {
			t.Errorf("%q: %v, want the usage", args, err)
		}
	}
	for _, args := range [][]string{{"--help"}, {"-h"}} {
		if _, err := ParseArgs(args); !errors.Is(err, ErrHelp) {
			t.Errorf("%q: %v, want ErrHelp", args, err)
		}
	}
}

// fakeOpener records what a command line asked of the app.
type fakeOpener struct{ calls []string }

func (f *fakeOpener) toggle()                { f.calls = append(f.calls, "toggle") }
func (f *fakeOpener) Show(c core.Category)   { f.calls = append(f.calls, "show "+string(c)) }
func (f *fakeOpener) openSettings() error    { f.calls = append(f.calls, "settings"); return nil }
func (f *fakeOpener) last() (calls []string) { calls, f.calls = f.calls, nil; return calls }

// As the Rust app's open: a second start with no option toggles the
// launcher, and its options act on the running app.
func TestEachStartActsOnItsCommandLine(t *testing.T) {
	for _, c := range []struct {
		launch  Launch
		running bool
		want    []string
	}{
		{Launch{}, false, []string{"show "}},
		{Launch{}, true, []string{"toggle"}},
		{Launch{Background: true}, false, nil},
		{Launch{Background: true}, true, nil},
		{Launch{Settings: true}, true, []string{"settings"}},
		{Launch{Settings: true}, false, []string{"settings"}},
		{Launch{Category: core.CategoryEmoji}, true, []string{"show emoji"}},
	} {
		f := &fakeOpener{}
		open(f, c.launch, c.running)
		if got := f.last(); !reflect.DeepEqual(got, c.want) {
			t.Errorf("%+v, running %v: %q, want %q", c.launch, c.running, got, c.want)
		}
	}
}

// Opening the running app from Finder or Spotlight shows the launcher,
// even when a blur just hid it, which a toggle would leave hidden.
func TestReopeningTheAppShowsTheLauncher(t *testing.T) {
	f := &fakeOpener{}
	reopen(f)
	if got := f.last(); !reflect.DeepEqual(got, []string{"show "}) {
		t.Errorf("%q, want the launcher shown", got)
	}
}

// The shortcut hides the launcher only while the user is in it; a
// launcher left open behind another window comes forward instead.
func TestTheShortcutHidesTheLauncherOnlyWhileItIsInUse(t *testing.T) {
	long := 2 * time.Second
	for _, c := range []struct {
		visible, focused bool
		sinceBlur        time.Duration
		want             toggleOutcome
	}{
		{true, true, long, hideLauncher},
		{true, false, long, showLauncher},
		{false, false, long, showLauncher},
		// X11: the press took the focus; with hide on blur it hid the
		// launcher, else it left it open without focus.
		{false, false, 50 * time.Millisecond, leaveLauncher},
		{true, false, 50 * time.Millisecond, hideLauncher},
	} {
		if got := toggleStep(c.visible, c.focused, c.sinceBlur); got != c.want {
			t.Errorf("visible %v focused %v %v after a blur: %d, want %d", c.visible, c.focused, c.sinceBlur, got, c.want)
		}
	}
}

// fakeTray records what wireTray did to the icon.
type fakeTray struct {
	menu         *mygo.Menu
	menus        []*mygo.Menu
	popped       []*mygo.Menu
	click, right func()
}

func (f *fakeTray) SetMenu(m *mygo.Menu)          { f.menu = m; f.menus = append(f.menus, m) }
func (f *fakeTray) PopUpMenu()                    { f.popped = append(f.popped, f.menu) }
func (f *fakeTray) OnClick(fn func()) func()      { f.click = fn; return func() {} }
func (f *fakeTray) OnRightClick(fn func()) func() { f.right = fn; return func() {} }

// The tray menu has Open, Settings…, and Quit, and a click on the icon
// opens the launcher, as in the Rust app.
func TestTheTrayOpensTheLauncherAndSettings(t *testing.T) {
	var ran []string
	record := func(name string) func() { return func() { ran = append(ran, name) } }
	items := trayMenu(record("show"), record("settings"), record("quit"))
	var labels []string
	for _, it := range items {
		labels = append(labels, it.Label)
		it.Click(it, nil)
	}
	if want := []string{"Open TinyDash", "Settings…", "Quit TinyDash"}; !reflect.DeepEqual(labels, want) {
		t.Errorf("labels %q, want %q", labels, want)
	}
	if want := []string{"show", "settings", "quit"}; !reflect.DeepEqual(ran, want) {
		t.Errorf("clicks ran %q, want %q", ran, want)
	}

	menu := &mygo.Menu{}
	for _, takesEveryClick := range []bool{false, true} {
		shown := 0
		tray := &fakeTray{}
		wireTray(tray, menu, func() { shown++ }, takesEveryClick)
		if tray.click == nil {
			t.Fatalf("takes every click %v: a click does nothing", takesEveryClick)
		}
		tray.click()
		if shown != 1 {
			t.Errorf("takes every click %v: a click showed the launcher %d times", takesEveryClick, shown)
		}
		if !takesEveryClick {
			if tray.menu != menu {
				t.Error("the icon has no menu")
			}
			continue
		}
		if tray.menu != nil || tray.right == nil {
			t.Fatalf("macOS: the menu %v would take the click, or a right-click opens nothing", tray.menu)
		}
		tray.right()
		if len(tray.popped) != 1 || tray.popped[0] != menu || tray.menu != nil {
			t.Errorf("macOS right-click: popped %v, menu left %v", tray.popped, tray.menu)
		}
	}
}

// A saved shortcut that blocks typing gives way to the default, with the
// Rust app's warning, which the launcher shows.
func TestASavedShortcutThatBlocksTypingGivesWayToTheDefault(t *testing.T) {
	keys := &shortcuts{}
	for _, stored := range []string{"A", "Shift+A", "Space", "Enter", "Nope+A", "KeyA", ""} {
		s := settings.Settings{Shortcut: stored}
		warning := checkedAtStartup(&s, keys.Parse)
		if s.Shortcut != "Control+Shift+Space" || !strings.HasSuffix(warning, " TinyDash uses Control+Shift+Space until you choose other keys in Settings > General.") {
			t.Errorf("%q: shortcut %q, warning %q", stored, s.Shortcut, warning)
		}
	}
	s := settings.Settings{Shortcut: "A"}
	if w := checkedAtStartup(&s, keys.Parse); !strings.HasPrefix(w, "“A” would block typing in every app.") {
		t.Errorf("warning %q", w)
	}
	for _, stored := range []string{"Control+Shift+Space", "Alt+Space", "Super+K", "F5", "Shift+F12", "Shift+Space"} {
		s := settings.Settings{Shortcut: stored}
		if w := checkedAtStartup(&s, keys.Parse); w != "" || s.Shortcut != stored {
			t.Errorf("%q: changed to %q, warning %q", stored, s.Shortcut, w)
		}
	}
}

// With Open at login on, each start registers this copy of TinyDash, so a
// login item left by the Rust app or an older copy points here.
func TestOpenAtLoginIsRegisteredAgainAtStartup(t *testing.T) {
	var calls []bool
	set := func(on bool) error { calls = append(calls, on); return nil }
	if err := keepOpenAtLogin(false, set); err != nil || calls != nil {
		t.Errorf("off: %v, calls %v", err, calls)
	}
	if err := keepOpenAtLogin(true, set); err != nil || !reflect.DeepEqual(calls, []bool{true}) {
		t.Errorf("on: %v, calls %v", err, calls)
	}
	err := keepOpenAtLogin(true, func(bool) error { return errors.New("no bundle") })
	if err == nil || !strings.Contains(err.Error(), "no bundle") {
		t.Errorf("failure: %v", err)
	}
}

// A hotkey runs while the user is in another app, so its error goes to a
// system notification.
func TestAHotkeyThatCannotRunSaysSoInANotification(t *testing.T) {
	var shown [][2]string
	notify := func(title, body string) error { shown = append(shown, [2]string{title, body}); return nil }
	reportHotkeyError(nil, notify)
	if shown != nil {
		t.Fatalf("a hotkey that ran notified %q", shown)
	}
	reportHotkeyError(errors.New("This item no longer exists."), notify)
	if want := [][2]string{{"The hotkey did not run", "This item no longer exists."}}; !reflect.DeepEqual(shown, want) {
		t.Errorf("%q, want %q", shown, want)
	}
}

// On Linux the error names the way out on Wayland, where no app can
// register a shortcut.
func TestARefusedShortcutGivesTheNextStep(t *testing.T) {
	if hint := shortcutTakenHint("linux"); !strings.Contains(hint, "On Wayland, assign a desktop shortcut that runs `tinydash` instead.") {
		t.Errorf("linux: %q", hint)
	}
	for _, goos := range []string{"darwin", "windows"} {
		if hint := shortcutTakenHint(goos); strings.Contains(hint, "Wayland") || !strings.Contains(hint, "choose other keys") {
			t.Errorf("%s: %q", goos, hint)
		}
	}
	err := registerError("Alt+Space", errors.New("taken"))
	if want := "Could not register Alt+Space: taken." + shortcutTaken; err.Error() != want {
		t.Errorf("%q, want %q", err, want)
	}
}

func TestTheDataOfTinyDash01IsPointedOut(t *testing.T) {
	config, data := t.TempDir(), t.TempDir()
	if w := legacyDataWarning(config, data); w != "" {
		t.Errorf("no old data: %q", w)
	}
	if err := os.WriteFile(filepath.Join(data, legacyDatabase), nil, 0o600); err != nil {
		t.Fatal(err)
	}
	w := legacyDataWarning(config, data)
	if !strings.HasPrefix(w, "This version starts with new history and pins. Data from TinyDash 0.1 is still in "+data+":") {
		t.Errorf("%q", w)
	}
}

// Rates download at startup when they are old, and after a try, the next
// waits an hour unless forced.
func TestRatesDownloadAtMostOnceAnHourUnlessForced(t *testing.T) {
	now := time.Date(2026, 11, 9, 12, 0, 0, 0, time.UTC)
	for _, c := range []struct {
		force, fresh bool
		triedAt      time.Time
		want         bool
	}{
		{false, false, time.Time{}, true},
		{false, true, time.Time{}, false},
		{false, false, now.Add(-time.Minute), false},
		{false, false, now.Add(-rateRetry), true},
		{true, true, now.Add(-time.Minute), true},
	} {
		if got := ratesDue(c.force, c.fresh, c.triedAt, now); got != c.want {
			t.Errorf("force %v fresh %v tried %v ago: %v, want %v", c.force, c.fresh, now.Sub(c.triedAt), got, c.want)
		}
	}
}

// The launcher opens in the exact center of the display with the pointer,
// or where the user dragged it while that spot is on a display.
func TestTheLauncherOpensCenteredOrWhereItWasDragged(t *testing.T) {
	primary := mygo.Display{Bounds: mygo.Rectangle{Width: 1440, Height: 900}, WorkArea: mygo.Rectangle{Y: 25, Width: 1440, Height: 875}}
	left := mygo.Display{Bounds: mygo.Rectangle{X: -1920, Width: 1920, Height: 1080}, WorkArea: mygo.Rectangle{X: -1920, Width: 1920, Height: 1080}}
	displays := []mygo.Display{primary, left}
	for _, c := range []struct {
		name    string
		saved   *settings.Position
		pointer mygo.Display
		want    mygo.Point
	}{
		{"centered", nil, primary, mygo.Point{X: 280, Y: 25 + 157}},
		{"on the display with the pointer", nil, left, mygo.Point{X: -1920 + 520, Y: 260}},
		{"dragged", &settings.Position{X: 40.4, Y: 600}, primary, mygo.Point{X: 40, Y: 600}},
		{"dragged to the other display", &settings.Position{X: -1800, Y: 10}, primary, mygo.Point{X: -1800, Y: 10}},
		{"dragged to a display that is gone", &settings.Position{X: 3000, Y: 10}, primary, mygo.Point{X: 280, Y: 182}},
		{"top edge off the display", &settings.Position{X: 40, Y: -5}, primary, mygo.Point{X: 280, Y: 182}},
	} {
		if got := launcherSpot(c.saved, panelWidth, panelHeight, displays, c.pointer); got != c.want {
			t.Errorf("%s: %+v, want %+v", c.name, got, c.want)
		}
	}
}
