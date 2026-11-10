package app

import (
	"fmt"
	"slices"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/settings"
	settingsview "github.com/jewei/tinydash/internal/ui/settings"
)

// settingsWindow is the open Settings window. Only the main thread reads
// and writes App.settings.
type settingsWindow struct {
	window *mygo.Window
	model  *settingsview.Model
}

// openSettings hides the launcher and shows the Settings window, which it
// creates on first use; one is open at a time. It runs from an action, off
// the main thread.
func (a *App) openSettings() error {
	mygo.RunOnMain(func() {
		a.panel.Hide()
		// TinyDash has no Dock icon, so it must bring itself to the front.
		defer mygo.App.Focus()
		if s := a.settings; s != nil {
			if s.window.IsMinimized() {
				s.window.Restore()
			}
			s.window.Show()
			s.window.Focus()
			return
		}
		model := settingsview.New((*settingsBackend)(a))
		window := mygo.NewWindow(mygo.WindowOptions{
			Title:     "TinyDash Settings",
			Width:     820,
			Height:    600,
			MinWidth:  640,
			MinHeight: 440,
			Content:   ui.View(model.View),
		})
		s := &settingsWindow{window, model}
		window.OnClosed(func() {
			// Closed while recording a shortcut: turn the shortcuts on again.
			model.Close()
			if a.settings == s {
				a.settings = nil
			}
		})
		a.settings = s
		window.Focus()
	})
	return nil
}

// settingsChanged reads the data again in an open Settings window, as
// after a change in the launcher. It runs on the main thread.
func (a *App) settingsChanged() {
	if s := a.settings; s != nil {
		s.window.Update(s.model.Stale)
	}
}

// toSettings runs a reply on the main thread and redraws Settings.
func (a *App) toSettings(reply func()) {
	mygo.RunOnMain(func() {
		reply()
		if s := a.settings; s != nil {
			s.window.Invalidate()
		}
	})
}

// applyToSystem applies the settings that change the OS: the launcher
// shortcut, the login item, and the tray icon. It runs on the writer,
// which calls it again from to to from to undo a change that failed.
func (a *App) applyToSystem(from, to settings.Settings) error {
	if to.Shortcut != from.Shortcut {
		if err := a.keys.SetLauncher(to.Shortcut); err != nil {
			return err
		}
	}
	if to.LaunchAtLogin != from.LaunchAtLogin {
		if err := mygo.App.SetOpenAtLogin(to.LaunchAtLogin); err != nil {
			return fmt.Errorf("Could not change open at login: %v. Try again, or change it in your system settings.", err)
		}
	}
	if to.ShowTrayIcon != from.ShowTrayIcon {
		return a.setTray(to.ShowTrayIcon)
	}
	return nil
}

// checkedAtStartup replaces a saved launcher shortcut that cannot be one,
// such as a plain letter in a hand-edited settings.json, with the default,
// and says why. The file keeps it until the next save.
func checkedAtStartup(s *settings.Settings, parse func(string) (engine.Keys, error)) string {
	keys, err := parse(s.Shortcut)
	if err == nil && !engine.CanOpenLauncher(keys) {
		err = fmt.Errorf("“%s” would block typing in every app. The launcher shortcut needs a modifier such as Control or Alt, or a function key.", s.Shortcut)
	}
	if err == nil {
		return ""
	}
	s.Shortcut = settings.Default().Shortcut
	return fmt.Sprintf("%v TinyDash uses %s until you choose other keys in Settings > General.", err, s.Shortcut)
}

// keepOpenAtLogin registers this copy of TinyDash as the login item while
// the setting is on. The Rust app's login item ran its own executable.
func keepOpenAtLogin(on bool, setOpenAtLogin func(bool) error) error {
	if !on {
		return nil
	}
	if err := setOpenAtLogin(true); err != nil {
		return fmt.Errorf("TinyDash could not set itself to open at login: %v. Turn Open at login off and on again in Settings > General.", err)
	}
	return nil
}

// settingsBackend is the Settings view's way into the app.
type settingsBackend App

func (b *settingsBackend) app() *App { return (*App)(b) }

func (b *settingsBackend) Settings() settings.Settings {
	return b.app().engine.Snapshot(time.Now()).Settings
}

func (b *settingsBackend) UpdateSettings(change func(*settings.Settings), done func(settings.Settings, error)) {
	a := b.app()
	go func() {
		before := a.engine.Snapshot(time.Now()).Settings
		err := a.actions.UpdateSettings(a.file, change, a.applyToSystem)
		after := a.engine.Snapshot(time.Now()).Settings
		if err == nil {
			if !slices.Equal(before.FileSearchFolders, after.FileSearchFolders) || !slices.Equal(before.FileSearchExcludedDirs, after.FileSearchExcludedDirs) {
				a.rebuild(a.files)
			}
			if after.CurrencyRatesEnabled && !before.CurrencyRatesEnabled {
				go a.refreshRates(true)
			}
			if after.ShowUpNext && !before.ShowUpNext {
				a.askCalendarAccess()
			}
		}
		a.toSettings(func() { done(after, err) })
	}()
}

func (b *settingsBackend) PauseShortcuts(paused bool, done func(error)) {
	a := b.app()
	go func() {
		err := a.keys.Pause(paused)
		a.toSettings(func() { done(err) })
	}()
}

func (b *settingsBackend) Library() []settingsview.LibraryItem {
	items := b.app().engine.Snapshot(time.Now()).Library.Items()
	out := make([]settingsview.LibraryItem, len(items))
	for i, it := range items {
		out[i] = settingsview.LibraryItem{ID: it.ID, Quicklink: it.Kind == library.KindQuicklink, Name: it.Name, Keyword: it.Keyword, Text: it.Text}
	}
	return out
}

func (b *settingsBackend) SaveLibraryItem(item settingsview.LibraryItem, done func(settingsview.LibraryItem, error)) {
	a := b.app()
	go func() {
		kind := library.KindSnippet
		if item.Quicklink {
			kind = library.KindQuicklink
		}
		saved, err := a.actions.SaveLibraryItem(library.Item{ID: item.ID, Kind: kind, Name: item.Name, Keyword: item.Keyword, Text: item.Text})
		reply := settingsview.LibraryItem{ID: saved.ID, Quicklink: item.Quicklink, Name: saved.Name, Keyword: saved.Keyword, Text: saved.Text}
		a.toSettings(func() { done(reply, err) })
	}()
}

func (b *settingsBackend) DeleteLibraryItem(id int64, done func(error)) {
	a := b.app()
	go func() {
		err := a.actions.DeleteLibraryItem(id)
		a.toSettings(func() { done(err) })
	}()
}

func (b *settingsBackend) Hidden() []engine.Listed {
	return engine.HiddenResults(b.app().engine.Snapshot(time.Now()))
}

func (b *settingsBackend) Unhide(id core.ID, done func(error)) {
	a := b.app()
	go func() {
		err := a.actions.Unhide(id)
		a.toSettings(func() { done(err) })
	}()
}

func (b *settingsBackend) ClearClipboard(done func(error)) {
	a := b.app()
	go func() {
		err := <-a.engine.ClearClipboard()
		a.toSettings(func() { done(err) })
	}()
}

func (b *settingsBackend) Aliases() []engine.Listed {
	return engine.AliasResults(b.app().engine.Snapshot(time.Now()))
}

func (b *settingsBackend) Hotkeys() []engine.Listed {
	return engine.HotkeyResults(b.app().engine.Snapshot(time.Now()))
}

// RemoveAlias and RemoveHotkey save an empty alias or hotkey, which removes
// it also when its item is gone.
func (b *settingsBackend) RemoveAlias(id core.ID, done func(error)) {
	b.runFromSettings(core.SetAlias{ID: id}, done)
}

func (b *settingsBackend) RemoveHotkey(id core.ID, done func(error)) {
	b.runFromSettings(core.SetHotkey{ID: id}, done)
}

func (b *settingsBackend) runFromSettings(action core.Action, done func(error)) {
	a := b.app()
	go func() {
		err := a.actions.Run(engine.FromLauncher, action, "")
		a.toSettings(func() { done(err) })
	}()
}

func (b *settingsBackend) About() settingsview.About {
	a := b.app()
	return settingsview.About{Version: mygo.App.Version(), Settings: a.config, Data: a.data}
}
