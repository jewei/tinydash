// Package app wires TinyDash together: the data folders, the engine, the
// launcher panel, the global shortcut and hotkeys, the tray icon, and the
// background work. It is the only package that holds mygo windows.
package app

import (
	"context"
	"crypto/rand"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"sync"
	"sync/atomic"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/feature/apps"
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
)

// Options say where the data lives and how TinyDash starts.
type Options struct {
	// DataDir, when set, holds both settings.json and tinydash.db, so a
	// development or verification run never touches the user's data.
	DataDir string
	// Hidden starts without showing the launcher, as at login.
	Hidden bool
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
	hud      *hud
	tray     *mygo.Tray
	settings *settingsWindow
	// home is the user's home folder, whose disk the disk widget shows.
	home string
	// mu guards builtAt and warnings.
	mu      sync.Mutex
	builtAt map[*engine.Slot]time.Time
	// blurHiddenAt is when blur last hid the panel; main thread only.
	blurHiddenAt time.Time
	// warnings wait here until the launcher shows them.
	warnings []string
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
	a := &App{ctx: ctx, home: dirs.Home, builtAt: map[*engine.Slot]time.Time{}, watch: newFileWatch()}

	s, warning, file := engine.LoadSettings(dirs.Config)
	a.file = file
	a.warn(warning)
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
	go a.engine.Run(ctx)
	a.desktop = &desktop{clip: NewClipboard(a.capture)}
	a.keys = &shortcuts{toggle: a.toggle, run: a.runHotkey}
	a.actions = engine.NewActions(a.engine, a.desktop, (*appShell)(a), a.keys, time.Now)
	a.more = a.newMoreFiles()
	a.watchFolders()

	a.apps = engine.NewSlot(a.engine.RebuildApps(a.listApps))
	a.files = engine.NewSlot(a.engine.RebuildFiles(a.fileDisk()))
	go a.apps.Run(ctx)
	go a.files.Run(ctx)
	a.rebuild(a.apps)
	a.rebuild(a.files)

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
	a.panel.OnBlur(func() {
		if a.engine.Snapshot(time.Now()).Settings.HideOnBlur {
			a.blurHiddenAt = time.Now()
			a.panel.Hide()
		}
	})

	a.warn(a.keys.SetLauncher(s.Shortcut))
	for id, err := range a.keys.Sync(first.Hotkeys) {
		a.warn(engine.HotkeyOff(&first, id, err))
	}
	a.warn(a.setTray(s.ShowTrayIcon))
	go a.watchClipboard()
	if !opts.Hidden {
		a.Show()
	}
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
	if a.panel != nil {
		a.panel.Update(func() {
			a.model.Stale()
			a.settingsChanged()
		})
	}
}

func (a *App) toggle() {
	if a.panel.IsVisible() {
		a.panel.Hide()
		return
	}
	if !blurWasThisPress(time.Since(a.blurHiddenAt)) {
		a.Show()
	}
}

// blurWithin is how soon after a blur hid the panel a shortcut press counts
// as the cause of that blur. Where the OS grabs the shortcut's keys (X11),
// the press takes focus from the panel, so blur hides it first; the press
// meant to hide it, and must not show it again.
const blurWithin = 400 * time.Millisecond

func blurWasThisPress(sinceBlurHide time.Duration) bool { return sinceBlurHide < blurWithin }

// Show puts the launcher on the display with the pointer, a third of the way
// down, and refreshes what is old.
func (a *App) Show() {
	d := mygo.Screen.DisplayNearestPoint(mygo.Screen.CursorScreenPoint())
	w, h := a.panel.Size()
	a.panel.SetPosition(d.WorkArea.X+(d.WorkArea.Width-w)/2, d.WorkArea.Y+(d.WorkArea.Height-h)/3)
	go a.desktop.clip.Count() // a copy made just before, in a password app, stays secret
	a.model.Show("")
	a.panel.Show()
	a.panel.Focus()
	a.rebuildIfOld(a.apps)
	a.rebuildIfOld(a.files)
	go a.refreshRates(false)
}

func (a *App) rebuild(slot *engine.Slot) {
	a.mu.Lock()
	a.builtAt[slot] = time.Now()
	a.mu.Unlock()
	if slot == a.files {
		a.watch.dirty.Store(false)
	}
	slot.Request()
}

func (a *App) rebuildIfOld(slot *engine.Slot) {
	a.mu.Lock()
	old := time.Since(a.builtAt[slot]) > staleAfter
	a.mu.Unlock()
	if old || slot == a.files && a.watch.dirty.Load() {
		a.rebuild(slot)
	}
}

func (a *App) runHotkey(id core.ID) {
	if err := a.actions.RunHotkey(id); err != nil {
		slog.Warn("hotkey", "id", id, "error", err)
	}
}

func (a *App) capture() clipboard.Capture {
	s := a.engine.Snapshot(time.Now()).Settings
	return clipboard.Capture{Images: s.ClipboardCaptureImages, Files: s.ClipboardCaptureFiles}
}

// watchClipboard saves what the user copies, twice a second.
func (a *App) watchClipboard() {
	monitor := engine.NewMonitor(a.engine, a.desktop.clip.Count, a.desktop.clip.Read)
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

func (a *App) listApps() ([]apps.App, error) {
	found, err := mygo.Shell.InstalledApps()
	if err != nil {
		return nil, err
	}
	byPath := make(map[string]mygo.InstalledApp, len(found))
	list := make([]apps.App, 0, len(found))
	for _, app := range found {
		byPath[app.Path] = app
		list = append(list, apps.App{Name: app.Name, Path: app.Path})
	}
	a.desktop.apps.Store(&byPath)
	return list, nil
}

func (a *App) fileDisk() engine.FileDisk {
	return engine.FileDisk{Disk: osDisk{}, Packages: platform.PackageExtensions, Hidden: platform.HiddenCheck()}
}

// refreshRates downloads the exchange rates while they are on and old, or
// always when forced.
func (a *App) refreshRates(force bool) {
	s := a.engine.Snapshot(time.Now())
	if !s.Settings.CurrencyRatesEnabled || (!force && s.Rates != nil && !s.Rates.IsStale(time.Now())) {
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
	tray.SetMenu(mygo.NewMenu([]*mygo.MenuItem{
		{Label: "Open TinyDash", Click: func(*mygo.MenuItem, *mygo.Window) { a.Show() }},
		mygo.Separator(),
		{Label: "Quit TinyDash", Click: func(*mygo.MenuItem, *mygo.Window) { mygo.App.Quit() }},
	}))
	a.tray = tray
	return nil
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
	return launcher.ViewSettings{Tabs: tabs, ClipboardHistory: s.ClipboardHistoryEnabled, CurrencyRates: s.CurrencyRatesEnabled, Theme: s.Theme}
}

func (b *backend) FileIcon(path string) *ui.Bitmap { return b.app().icons.get(path, b.app().panel) }

func (b *backend) PauseShortcuts(paused bool) error { return b.app().keys.Pause(paused) }

func (b *backend) EnableClipboardHistory(done func(error)) {
	go func() {
		err := <-b.app().engine.UpdateSettings(b.app().file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = true })
		b.app().panel.Update(func() { done(err) })
	}()
}

func (b *backend) Hide() { b.app().panel.Hide() }

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
	s.app().panel.Show()
	return nil
}

func (s *appShell) ShowHUD(text string) { s.app().hud.show(text) }

func (s *appShell) OpenSettings() error { return s.app().openSettings() }

func (s *appShell) CenterLauncher() error {
	return <-s.app().engine.UpdateSettings(s.app().file, func(st *settings.Settings) { st.LauncherPosition = nil })
}

func (s *appShell) TypeQuery(text string) {
	s.app().panel.Update(func() { s.app().model.TypeQuery(text) })
}

func (s *appShell) Refresh() {
	s.app().rebuild(s.app().apps)
	s.app().rebuild(s.app().files)
	go s.app().refreshRates(true)
}

func (s *appShell) TurnOnCurrencyRates() error {
	if err := <-s.app().engine.UpdateSettings(s.app().file, func(st *settings.Settings) { st.CurrencyRatesEnabled = true }); err != nil {
		return err
	}
	go s.app().refreshRates(true)
	return nil
}

func (s *appShell) Quit() { mygo.App.Quit() }

// iconCache loads file icons off the main thread and keeps them by path.
type iconCache struct {
	mu      sync.Mutex
	icons   map[string]*ui.Bitmap
	loading map[string]bool
}

func (c *iconCache) get(path string, window *mygo.Window) *ui.Bitmap {
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
		var icon *ui.Bitmap
		if data, err := mygo.Shell.FileIcon(path, 64); err == nil {
			icon, _ = ui.DecodeBitmap(data)
		}
		c.mu.Lock()
		c.icons[path] = icon
		delete(c.loading, path)
		c.mu.Unlock()
		window.Update(func() {})
	}()
	return nil
}
