// Package settings is the Settings window's native view and its model:
// General, Widgets, Clipboard, Search, Snippets, and Quicklinks. It owns no
// window: the app creates one, shows the view, and implements Backend.
//
// Changes save at once. Each change is a function of the saved settings,
// sent one at a time, so a change that failed is never sent again with the
// next one; the form shows the saved settings with the changes still on
// their way on top.
package settings

import (
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/settings"
)

// Backend is what Settings needs from the app. Every method runs on the
// main thread, and every done function must be called there too (through
// Window.Update), once.
type Backend interface {
	// Settings are the saved settings. Settings reads them as it opens
	// and again on Stale, as when the launcher changed one.
	Settings() settings.Settings
	// UpdateSettings applies change to the saved settings, applies what
	// changes the OS, saves, and calls done with the saved settings. A
	// change can fail, as a shortcut another app holds, and the form must
	// show why and the value that stayed, so the outcome comes back.
	UpdateSettings(change func(*settings.Settings), done func(settings.Settings, error))
	// PauseShortcuts turns the launcher shortcut and every hotkey off
	// while the shortcut recorder records, so pressing the shortcut that
	// is set records it instead of showing the launcher, and on again
	// after; done as for UpdateSettings. It waits for a settings change
	// that holds the shortcuts, which may itself wait for the main thread,
	// so it must not block.
	PauseShortcuts(paused bool, done func(error))
	// Library is every snippet and quicklink. Settings reads it as it
	// opens, after each save or delete, and on Stale.
	Library() []LibraryItem
	// SaveLibraryItem checks an item, saves it, and calls done with the
	// item as saved, with its ID, or with why it was refused.
	SaveLibraryItem(item LibraryItem, done func(LibraryItem, error))
	// DeleteLibraryItem deletes an item with its pin, alias, and hotkey.
	DeleteLibraryItem(id int64, done func(error))
	// Hidden lists the results the user hid, by title.
	Hidden() []engine.Listed
	// Unhide shows a hidden result in search again.
	Unhide(id core.ID, done func(error))
	// Aliases lists the aliases the user set, by alias; Value is the alias.
	Aliases() []engine.Listed
	// RemoveAlias removes a result's alias, also when its item is gone.
	RemoveAlias(id core.ID, done func(error))
	// Hotkeys lists the hotkeys the user set, by title; Value is the
	// accelerator text.
	Hotkeys() []engine.Listed
	// RemoveHotkey removes a result's hotkey, also when its item is gone.
	RemoveHotkey(id core.ID, done func(error))
	// ClearClipboard deletes the clipboard history except pins.
	ClearClipboard(done func(error))
}

// LibraryItem is a snippet or a quicklink as the editor holds it.
type LibraryItem struct {
	// ID is 0 for an item that is not saved yet.
	ID        int64
	Quicklink bool
	Name      string
	// Keyword is optional: one word that finds the item.
	Keyword string
	// Text is the snippet's text, or the quicklink's URL or path.
	Text string
}
