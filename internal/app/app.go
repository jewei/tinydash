// Package app wires TinyDash together: the data folders, the engine, the
// launcher panel, the global shortcut and hotkeys, the tray icon, and the
// background work. It is the only package that holds mygo windows.
package app

import (
	"context"
	"crypto/rand"
	"fmt"
	"log/slog"
	"math"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
	"github.com/jewei/tinydash/internal/ui/kit"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// Identifier names the user's data folders. It is the Rust app's, so the
// Go app reads the same settings, history, and pins.
const Identifier = "dev.tinydash.launcher"

const (
	panelWidth, panelHeight = 880, 560
	// staleAfter: an index older than this is rebuilt when the launcher
	// shows. File events say sooner (fileWatch); this bounds how old it
	// gets where they cannot.
	staleAfter = 15 * time.Minute
	tick       = 500 * time.Millisecond
	ratesURL   = "https://api.frankfurter.dev/v1/latest"
	// rateRetry: after a rate download, the next one waits this long
	// unless forced, so a failing one does not run at each show.
	rateRetry = time.Hour
	// dragSettle: the launcher's spot is saved once a drag has not moved
	// it for this long, so a drag writes settings.json once.
	dragSettle = 300 * time.Millisecond
)

// Options say where the data lives and how TinyDash starts.
type Options struct {
	// DataDir, when set, holds both settings.json and tinydash.db, so a
	// development or verification run never touches the user's data.
	DataDir string
	// Launch says which window to open, if any.
	Launch Launch
}

// App is a running TinyDash.
type App struct {
	ctx      context.Context
	engine   *engine.Engine
	actions  *engine.Actions
	file     engine.SettingsFile
	desktop  *desktop
	keys     *shortcuts
	model    *launcher.Model
	panel    *mygo.Window
	apps     *engine.Slot
	files    *engine.Slot
	icons    iconCache
	more     *moreFiles
	watch    *fileWatch
	appWatch *fileWatch
	hud      *hud
	tray     *mygo.Tray
	settings *settingsWindow
	// home is the user's home folder, whose disk the disk widget shows.
	home string
	// config and data hold settings.json and tinydash.db; About shows them.
	config, data string
	// mu guards builtAt, ratesTriedAt, and warnings.
	mu      sync.Mutex
	builtAt map[*engine.Slot]time.Time
	// ratesTriedAt is when the last rate download started.
	ratesTriedAt time.Time
	// blurredAt is when the panel last lost focus; main thread only.
	blurredAt time.Time
	// movedAt is when the panel last moved, as in a drag, and dragStartedAt
	// when a press on the tab bar or footer began one, or zero once it was
	// let go of; main thread only.
	movedAt, dragStartedAt time.Time
	// placed is where TinyDash last put the panel, and dragSave saves
	// where the user dragged it; main thread only.
	placed   mygo.Point
	dragSave *time.Timer
	// warnings wait here until the launcher shows them.
	warnings []string
	weather  weatherState
}

// Start loads the user's data and starts TinyDash. Call it once mygo is
// ready (mygo.App.WhenReady). Problems that leave TinyDash usable become
// warnings in the log; only missing data folders stop it.
func Start(ctx context.Context, opts Options) (*App, error) {
	dirs, err := platform.AppDirs(Identifier)
	if err != nil {
		return nil, err
	}
	if opts.DataDir != "" {
		dirs.Config, dirs.Data = opts.DataDir, opts.DataDir
	}
	a := &App{ctx: ctx, home: dirs.Home, config: dirs.Config, data: dirs.Data, builtAt: map[*engine.Slot]time.Time{}, watch: newFileWatch(), appWatch: newAppWatch()}

	s, warning, file := engine.LoadSettings(dirs.Config)
	a.file = file
	a.warn(warning)
	a.keys = &shortcuts{os: mygo.GlobalShortcut, toggle: a.toggle, run: a.runHotkey}
	a.warn(checkedAtStartup(&s, a.keys.Parse))
	a.warn(legacyDataWarning(dirs.Config, dirs.Data))
	db, warning, err := store.Open(ctx, dirs.Data, restrictToOwner)
	a.warn(warning)
	if err != nil {
		a.warn(fmt.Sprintf("TinyDash could not open its data (%v), so this session keeps nothing.", err))
		if db, err = store.OpenMemory(ctx); err != nil {
			return nil, err
		}
	}
	first, warnings := engine.Load(ctx, db, s, dirs.Home, rand.Reader)
	for _, w := range warnings {
		a.warn(w)
	}

	a.engine = engine.New(db, first, a.dataChanged)
	a.desktop = &desktop{clip: NewClipboard(a.capture)}
	a.actions = engine.NewActions(a.engine, a.desktop, (*appShell)(a), a.keys, time.Now)
	a.more = a.newMoreFiles()

	// Without its fonts, text falls back to the system's, which still reads.
	if err := kit.RegisterFonts(); err != nil {
		slog.Warn("fonts", "error", err)
	}
	a.model = launcher.New((*backend)(a))
	a.panel = mygo.NewWindow(mygo.WindowOptions{
		Title:         "TinyDash",
		Panel:         true,
		Hidden:        true,
		Frameless:     true,
		CornerRadius:  16,
		Width:         panelWidth,
		Height:        panelHeight,
		DisableResize: true,
		Content:       ui.View(a.model.View),
	})
	// Escape, Cmd-W, and Alt-F4 hide the panel, so it shows again at once.
	// Quitting closes it for good: mygo quits on SIGTERM by closing windows,
	// and a close the panel always refused would cancel the quit.
	var quitting atomic.Bool
	mygo.App.OnBeforeQuit(func(*mygo.QuitEvent) { quitting.Store(true) })
	a.panel.OnClose(func(e *mygo.CloseEvent) {
		if quitting.Load() {
			return
		}
		e.PreventDefault()
		a.panel.Hide()
	})
	a.hud = newHUD(a.panel)
	a.icons.load, a.icons.loaded = loadFileIcon, func() { a.panel.Update(func() {}) }
	// The writer and the slots tell the panel and the model about changes,
	// so they start once both exist.
	go a.engine.Run(ctx)
	a.watchFolders()
	a.apps = engine.NewSlot(a.engine.RebuildApps(installedApps))
	a.files = engine.NewSlot(a.engine.RebuildFiles(a.fileDisk()))
	go a.apps.Run(ctx)
	go a.files.Run(ctx)
	a.rebuild(a.apps)
	a.rebuild(a.files)

	a.panel.OnBlur(func() {
		a.blurredAt = time.Now()
		if a.engine.Snapshot(time.Now()).Settings.HideOnBlur {
			a.hideUnlessDragged()
		}
	})
	a.panel.OnMove(a.panelMoved)
	a.panel.OnHide(a.model.Hidden)

	a.warn(a.keys.SetLauncher(s.Shortcut))
	for id, err := range a.keys.Sync(first.Hotkeys) {
		a.warn(engine.HotkeyOff(&first, id, err))
	}
	a.warn(a.setTray(s.ShowTrayIcon))
	// The login item may still be the Rust app's, or point at an older
	// copy of TinyDash: register this one.
	a.warn(keepOpenAtLogin(s.LaunchAtLogin, mygo.App.SetOpenAtLogin))
	go a.watchClipboard()
	go a.refreshRates(false)
	go a.refreshWeather(false)
	open(a, opts.Launch, false)
	return a, nil
}

func (a *App) warn(message any) {
	switch m := message.(type) {
	case string:
		if m == "" {
			return
		}
		a.mu.Lock()
		a.warnings = append(a.warnings, m)
		a.mu.Unlock()
		slog.Warn(m)
	case error:
		if m != nil {
			a.warn(m.Error())
		}
	}
}

// dataChanged runs on the writer after each change: the view searches
// again, and Settings reads the data again.
func (a *App) dataChanged() {
	a.watchFolders()
	a.panel.Update(func() {
		a.model.Stale()
		a.settingsChanged()
	})
}

func (a *App) toggle() {
	switch toggleStep(a.panel.IsVisible(), a.panel.IsFocused(), time.Since(a.blurredAt)) {
	case hideLauncher:
		a.panel.Hide()
	case showLauncher:
		a.Show("")
	}
}

type toggleOutcome uint8

const (
	leaveLauncher toggleOutcome = iota
	hideLauncher
	showLauncher
)

// toggleStep is what the launcher shortcut does: it hides the launcher
// only while the user is in it, that is, visible and focused, and else
// shows it, as the Rust app did. Where the OS grabs the shortcut's keys
// (X11), the press itself takes focus from the panel, and with hide on
// blur hides it first. A blur within blurWithin was that press: it meant
// to hide the launcher, so the launcher hides, or stays hidden.
func toggleStep(visible, focused bool, sinceBlur time.Duration) toggleOutcome {
	pressBlurred := blurWasThisPress(sinceBlur)
	switch {
	case visible && (focused || pressBlurred):
		return hideLauncher
	case !visible && pressBlurred:
		return leaveLauncher
	default:
		return showLauncher
	}
}

// blurWithin is how soon after a blur a shortcut press counts as its cause.
const blurWithin = 400 * time.Millisecond

func blurWasThisPress(sinceBlur time.Duration) bool { return sinceBlur < blurWithin }

// Show puts the launcher where the user last dragged it, or in the center
// of the display with the pointer, on category with an empty query, and
// refreshes what is old.
func (a *App) Show(category core.Category) {
	a.place(a.engine.Snapshot(time.Now()).Settings.LauncherPosition)
	go a.desktop.clip.Count() // a copy made just before, in a password app, stays secret
	a.model.Show(category)
	a.panel.Show()
	a.panel.Focus()
	a.rebuildIfOld(a.apps)
	a.rebuildIfOld(a.files)
	go a.refreshRates(false)
	go a.refreshWeather(false)
}

// place moves the panel to saved, or centers it; main thread only.
func (a *App) place(saved *settings.Position) {
	w, h := a.panel.Size()
	pointer := mygo.Screen.DisplayNearestPoint(mygo.Screen.CursorScreenPoint())
	a.placed = launcherSpot(saved, w, h, mygo.Screen.Displays(), pointer)
	a.panel.SetPosition(a.placed.X, a.placed.Y)
}

// launcherSpot is where the launcher of size w × h opens: at saved while
// the middle of its top edge is on a display, so a display that is gone
// does not hide it, else in the exact center of the work area of the
// display with the pointer. It has a fixed height, so it does not sit
// high, as launchers that grow do.
func launcherSpot(saved *settings.Position, w, h int, displays []mygo.Display, pointer mygo.Display) mygo.Point {
	if saved != nil {
		x, y := int(math.Round(saved.X)), int(math.Round(saved.Y))
		for _, d := range displays {
			if contains(d.Bounds, x+w/2, y+1) {
				return mygo.Point{X: x, Y: y}
			}
		}
	}
	area := pointer.WorkArea
	return mygo.Point{X: area.X + max(area.Width-w, 0)/2, Y: area.Y + max(area.Height-h, 0)/2}
}

func contains(r mygo.Rectangle, x, y int) bool {
	return x >= r.X && x < r.X+r.Width && y >= r.Y && y < r.Y+r.Height
}

// panelMoved saves where the user dragged the panel, once the drag
// settles. TinyDash's own moves go to placed, so they save nothing.
// hideUnlessDragged hides the panel after a blur, unless the blur came
// from a drag: where the window manager moves windows itself (X11), the
// move takes focus from the panel before the first move event, so the
// decision waits a moment and waits again while the panel moves.
func (a *App) hideUnlessDragged() {
	time.AfterFunc(blurGrace, func() {
		mygo.RunOnMain(func() {
			if !a.panel.IsVisible() {
				return
			}
			switch blurOutcome(a.panel.IsFocused(), time.Since(a.movedAt), time.Since(a.dragStartedAt), a.movedAt.After(a.dragStartedAt)) {
			case hideAfterBlur:
				a.panel.Hide()
			case waitForDrag:
				a.hideUnlessDragged()
			}
		})
	})
}

// blurGrace is how long a blur waits for a move that shows it was a drag.
const blurGrace = 250 * time.Millisecond

type blurStep uint8

const (
	stayAfterBlur blurStep = iota
	waitForDrag
	hideAfterBlur
)

// dragLimit bounds how long a drag that reports no move holds off a hide.
// Where the window manager moves the panel (X11), mygo reports the move
// only as the drag ends, so a drag in progress has no other sign.
const dragLimit = 10 * time.Second

// blurOutcome decides a blur once blurGrace passed. A panel that has the
// focus again stays. A drag that started and has not moved yet, or a panel
// that moved within blurGrace, is still being dragged. Any other hides: a
// press on a bar that was let go of (DragEnded) is no drag.
func blurOutcome(focused bool, sinceMove, sinceDrag time.Duration, movedSinceDrag bool) blurStep {
	switch {
	case focused:
		return stayAfterBlur
	case sinceDrag < dragLimit && !movedSinceDrag, sinceMove < blurGrace:
		return waitForDrag
	}
	return hideAfterBlur
}

func (a *App) panelMoved() {
	a.movedAt = time.Now()
	if !a.panel.IsVisible() {
		return
	}
	x, y := a.panel.Position()
	if (mygo.Point{X: x, Y: y}) == a.placed {
		return
	}
	if a.dragSave != nil {
		a.dragSave.Stop()
	}
	a.dragSave = time.AfterFunc(dragSettle, a.saveDraggedSpot)
}

func (a *App) saveDraggedSpot() {
	x, y := a.panel.Position()
	spot := &settings.Position{X: float64(x), Y: float64(y)}
	err := <-a.engine.UpdateSettings(a.file, func(s *settings.Settings) {
		if s.LauncherPosition == nil || *s.LauncherPosition != *spot {
			s.LauncherPosition = spot
		}
	})
	if err != nil {
		slog.Warn("Could not save where the launcher is", "error", err)
	}
}

func (a *App) rebuild(slot *engine.Slot) {
	a.mu.Lock()
	a.builtAt[slot] = time.Now()
	a.mu.Unlock()
	if w := a.watchOf(slot); w != nil {
		w.dirty.Store(false)
	}
	slot.Request()
}

func (a *App) rebuildIfOld(slot *engine.Slot) {
	a.mu.Lock()
	old := time.Since(a.builtAt[slot]) > staleAfter
	a.mu.Unlock()
	if old || a.changed(slot) {
		a.rebuild(slot)
	}
}

func (a *App) runHotkey(id core.ID) {
	reportHotkeyError(a.actions.RunHotkey(id), showNotification)
}

// reportHotkeyError shows why a hotkey did not run in a system
// notification: the user is in another app, and the "Copied" message is
// too small for an error and shows a check mark.
func reportHotkeyError(err error, notify func(title, body string) error) {
	if err == nil {
		return
	}
	slog.Warn("A hotkey did not run", "error", err)
	if err := notify("The hotkey did not run", err.Error()); err != nil {
		slog.Warn("Could not show why the hotkey did not run", "error", err)
	}
}

func showNotification(title, body string) error {
	return mygo.NewNotification(mygo.NotificationOptions{Title: title, Body: body}).Show()
}

func (a *App) capture() clipboard.Capture {
	s := a.engine.Snapshot(time.Now()).Settings
	return clipboard.Capture{Images: s.ClipboardCaptureImages, Files: s.ClipboardCaptureFiles}
}

// watchClipboard saves what the user copies, twice a second.
func (a *App) watchClipboard() {
	monitor := engine.NewMonitor(a.engine, a.desktop.clip.Count, a.desktop.clip.Capture)
	ticker := time.NewTicker(tick)
	defer ticker.Stop()
	for {
		select {
		case <-a.ctx.Done():
			return
		case now := <-ticker.C:
			if done := monitor.Tick(now.Unix()); done != nil {
				if err := <-done; err != nil {
					slog.Warn("clipboard history", "error", err)
				}
			}
		}
	}
}

func (a *App) fileDisk() engine.FileDisk {
	return engine.FileDisk{Disk: osDisk{}, Packages: platform.PackageExtensions, Hidden: platform.HiddenCheck()}
}

// refreshRates downloads the exchange rates while they are on and old, at
// most once per rateRetry, or always when forced.
func (a *App) refreshRates(force bool) {
	now := time.Now()
	s := a.engine.Snapshot(now)
	if !s.Settings.CurrencyRatesEnabled {
		return
	}
	a.mu.Lock()
	due := ratesDue(force, s.Rates != nil && !s.Rates.IsStale(now), a.ratesTriedAt, now)
	if due {
		a.ratesTriedAt = now
	}
	a.mu.Unlock()
	if !due {
		return
	}
	ctx, cancel := context.WithTimeout(a.ctx, 10*time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, ratesURL, nil)
	if err != nil {
		return
	}
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		slog.Warn("currency rates", "error", err)
		return
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		slog.Warn("currency rates", "status", response.Status)
		return
	}
	rates, err := currency.Parse(response.Body, time.Now())
	if err != nil {
		slog.Warn("currency rates", "error", err)
		return
	}
	<-a.engine.SetRates(rates)
}

// ratesDue reports whether to download rates now: when forced, or when
// they are missing or old and the last try was rateRetry ago or more.
func ratesDue(force, fresh bool, triedAt, now time.Time) bool {
	return force || !fresh && now.Sub(triedAt) >= rateRetry
}

// setTray shows or removes the tray icon. Start calls it before Settings
// can open, and then only the writer does, so calls never overlap.
func (a *App) setTray(visible bool) error {
	if !visible {
		if a.tray != nil {
			a.tray.Destroy()
			a.tray = nil
		}
		return nil
	}
	if a.tray != nil {
		return nil
	}
	tray, err := mygo.NewTray(mygo.TrayOptions{ID: "tinydash", Icon: trayIcon(), IconIsTemplate: templateTrayIcon, ToolTip: "TinyDash"})
	if err != nil {
		return fmt.Errorf("The tray icon is unavailable: %v. Open TinyDash with its shortcut instead.", err)
	}
	show := func() { a.Show("") }
	openSettings := func() {
		if err := a.openSettings(); err != nil {
			slog.Warn("open settings", "error", err)
		}
	}
	wireTray(tray, mygo.NewMenu(trayMenu(show, openSettings, mygo.App.Quit)), show, menuTakesEveryTrayClick)
	a.tray = tray
	return nil
}

// trayMenu is the tray icon's menu, as the Rust app had it.
func trayMenu(show, settings, quit func()) []*mygo.MenuItem {
	item := func(label string, fn func()) *mygo.MenuItem {
		return &mygo.MenuItem{Label: label, Click: func(*mygo.MenuItem, *mygo.Window) { fn() }}
	}
	return []*mygo.MenuItem{item("Open TinyDash", show), item("Settings…", settings), item("Quit TinyDash", quit)}
}

// trayHandle is the part of mygo.Tray that wireTray uses.
type trayHandle interface {
	SetMenu(*mygo.Menu)
	PopUpMenu()
	OnClick(func()) func()
	OnRightClick(func()) func()
}

// wireTray makes a click on the icon show the launcher and a right-click
// open the menu. Where the menu would take every click (macOS), the icon
// holds the menu only while a right-click shows it.
func wireTray(tray trayHandle, menu *mygo.Menu, show func(), menuTakesEveryClick bool) {
	tray.OnClick(show)
	if !menuTakesEveryClick {
		tray.SetMenu(menu)
		return
	}
	tray.OnRightClick(func() {
		tray.SetMenu(menu)
		tray.PopUpMenu()
		tray.SetMenu(nil)
	})
}

// legacyDatabase is the database of TinyDash 0.1, which TinyDash does not
// read.
const legacyDatabase = "tinydash.sqlite3"

// legacyDataWarning says where TinyDash 0.1 left its data, if it did. The
// Rust app looked in its roaming data folder, which is the config folder
// on Windows and the data folder elsewhere.
func legacyDataWarning(folders ...string) string {
	for _, folder := range folders {
		if _, err := os.Stat(filepath.Join(folder, legacyDatabase)); err == nil {
			return fmt.Sprintf("This version starts with new history and pins. Data from TinyDash 0.1 is still in %s: delete %s and recovery.tar there if you no longer need it.", folder, legacyDatabase)
		}
	}
	return ""
}

// restrictToOwner keeps tinydash.db, which can hold private clipboard text,
// readable by its owner only.
func restrictToOwner(path string) error { return os.Chmod(path, 0o600) }

// backend is the launcher view's way into the app.
type backend App

func (b *backend) app() *App { return (*App)(b) }

func (b *backend) Search(query string, category core.Category) []core.Result {
	s := b.app().engine.Snapshot(time.Now())
	return append(search.Search(s, query, category), b.app().more.results(b.app().ctx, s, query, category)...)
}

func (b *backend) Run(action core.Action, from core.ID, done func(error)) {
	go func() {
		err := b.app().actions.Run(engine.FromLauncher, action, from)
		b.app().panel.Update(func() { done(err) })
	}()
}

func (b *backend) Settings() launcher.ViewSettings {
	s := b.app().engine.Snapshot(time.Now()).Settings
	var tabs []core.Category
	for _, tab := range s.Tabs {
		if tab.Shown {
			tabs = append(tabs, tab.Category)
		}
	}
	return launcher.ViewSettings{Tabs: tabs, ClipboardHistory: s.ClipboardHistoryEnabled, CurrencyRates: s.CurrencyRatesEnabled, Theme: s.Theme,
		Weather: s.ShowWeather && strings.TrimSpace(s.WeatherCity) != ""}
}

func (b *backend) FileIcon(path string) *ui.Bitmap { return b.app().icons.get(path) }

func (b *backend) PauseShortcuts(paused bool) error { return b.app().keys.Pause(paused) }

func (b *backend) EnableClipboardHistory(done func(error)) {
	go func() {
		err := <-b.app().engine.UpdateSettings(b.app().file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = true })
		b.app().panel.Update(func() { done(err) })
	}()
}

func (b *backend) Hide() { b.app().panel.Hide() }

func (b *backend) DragStarted() { b.app().dragStartedAt = time.Now() }

func (b *backend) DragEnded() { b.app().dragStartedAt = time.Time{} }

// appShell is what actions ask of TinyDash's own windows.
type appShell App

func (s *appShell) app() *App { return (*App)(s) }

func (s *appShell) Hide() error {
	s.app().panel.Hide()
	return nil
}

// Dismiss hides the panel; the app the user came from has the keyboard
// again, because the panel never took it from that app.
func (s *appShell) Dismiss() error { return s.Hide() }

func (s *appShell) ShowAgain() error {
	a := s.app()
	// The model is main-thread state; this runs on an action's goroutine.
	a.panel.Update(a.model.ShownAgain)
	a.panel.Show()
	return nil
}

func (s *appShell) ShowHUD(text string) { s.app().hud.show(text) }

func (s *appShell) OpenSettings() error { return s.app().openSettings() }

// CenterLauncher forgets where the user dragged the launcher and centers
// it now.
func (s *appShell) CenterLauncher() error {
	a := s.app()
	mygo.RunOnMain(func() {
		if a.dragSave != nil {
			a.dragSave.Stop()
		}
	})
	if err := <-a.engine.UpdateSettings(a.file, func(st *settings.Settings) { st.LauncherPosition = nil }); err != nil {
		return err
	}
	mygo.RunOnMain(func() { a.place(nil) })
	return nil
}

func (s *appShell) TypeQuery(text string) {
	s.app().panel.Update(func() { s.app().model.TypeQuery(text) })
}

func (s *appShell) Refresh() {
	s.app().rebuild(s.app().apps)
	s.app().rebuild(s.app().files)
	go s.app().refreshRates(true)
	go s.app().refreshWeather(true)
}

func (s *appShell) TurnOnCurrencyRates() error {
	if err := <-s.app().engine.UpdateSettings(s.app().file, func(st *settings.Settings) { st.CurrencyRatesEnabled = true }); err != nil {
		return err
	}
	go s.app().refreshRates(true)
	return nil
}

func (s *appShell) Quit() { mygo.App.Quit() }

// maxFileIcons bounds the file icons kept: a 64-pixel icon takes 16 KB,
// so the cache stays under 8 MB.
const maxFileIcons = 512

// iconCache loads file icons off the main thread and keeps the newest
// maxFileIcons by path.
type iconCache struct {
	// load reads a file's icon, or nil; it runs off the main thread.
	load func(path string) *ui.Bitmap
	// loaded asks the panel to draw again once an icon came.
	loaded func()

	mu    sync.Mutex
	icons map[string]*ui.Bitmap
	// order holds the paths of icons, oldest first.
	order   []string
	loading map[string]bool
}

func (c *iconCache) get(path string) *ui.Bitmap {
	c.mu.Lock()
	defer c.mu.Unlock()
	if icon, ok := c.icons[path]; ok {
		return icon
	}
	if c.loading[path] {
		return nil
	}
	if c.loading == nil {
		c.loading, c.icons = map[string]bool{}, map[string]*ui.Bitmap{}
	}
	c.loading[path] = true
	go func() {
		icon := c.load(path)
		c.mu.Lock()
		if len(c.order) == maxFileIcons {
			delete(c.icons, c.order[0])
			c.order = c.order[1:]
		}
		c.icons[path] = icon
		c.order = append(c.order, path)
		delete(c.loading, path)
		c.mu.Unlock()
		c.loaded()
	}()
	return nil
}

// loadFileIcon is the system icon of the file at path, or nil.
func loadFileIcon(path string) *ui.Bitmap {
	data, err := mygo.Shell.FileIcon(path, 64)
	if err != nil {
		return nil
	}
	icon, _ := ui.DecodeBitmap(data)
	return icon
}
