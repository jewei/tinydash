// Package system lists the System tab's commands: power, session, and
// TinyDash's own.
package system

import (
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// Context is how one search ranks and shows its results.
type Context struct {
	Personal core.Personal
	// Available is the OS commands this machine can run. Nil offers every
	// command, which is macOS, Windows, and tests. An empty map offers
	// none. Commands that are not OS commands are always offered.
	Available map[core.SystemCommand]bool
}

func (c Context) boost(id core.ID) uint32 {
	if c.Personal == nil {
		return 0
	}
	return c.Personal.Boost(id)
}

func (c Context) pinned(id core.ID) bool { return c.Personal != nil && c.Personal.Pinned(id) }

// offers reports whether this machine can run cmd. A nil Available offers
// every OS command. A command that stays inside TinyDash, such as Quit, is
// always offered.
func (c Context) offers(cmd *command) bool {
	action, ok := cmd.action.(core.System)
	if !ok || c.Available == nil {
		return true
	}
	return c.Available[action.Command]
}

type command struct {
	// key is the stable part of the result ID "system:<key>".
	key         string
	name        string
	description string
	aliases     []string
	symbol      core.Symbol
	action      core.Action
	// confirm: commands that end the session or delete data ask first.
	confirm string
}

func run(c core.SystemCommand) core.Action { return core.System{Command: c} }

var commands = []command{
	{"lock", "Lock Screen", "Lock this computer", []string{"lock"}, core.SymbolLock, run(core.CommandLock), ""},
	{"sleep", "Sleep", "Put the computer to sleep", []string{"suspend"}, core.SymbolMoon, run(core.CommandSleep), "Put the computer to sleep?"},
	{"restart", "Restart", "Restart the computer", []string{"reboot"}, core.SymbolRestart, run(core.CommandRestart), confirmRestart},
	{"shut-down", "Shut Down", "Turn off the computer", []string{"shutdown", "power off", "turn off"}, core.SymbolPower, run(core.CommandShutDown), confirmShutDown},
	{"log-out", "Log Out", "End this session", []string{"logout", "sign out"}, core.SymbolLogOut, run(core.CommandLogOut), "Log out? Apps will be asked to quit."},
	{"empty-trash", emptyTrash, "Permanently delete trashed items", []string{"empty trash", "empty recycle bin", "trash", "recycle bin"}, core.SymbolTrash, run(core.CommandEmptyTrash), "Permanently delete everything in the trash? This cannot be undone."},
	{"open-trash", openTrash, "Show the items in the trash", []string{"trash", "recycle bin", "bin"}, core.SymbolTrash, run(core.CommandOpenTrash), ""},
	{"sleep-displays", sleepDisplays, "Turn off the screens; the computer keeps running", []string{"display off", "screen off", "monitor off", "sleep displays"}, core.SymbolDisplay, run(core.CommandSleepDisplays), ""},
	{"toggle-dark-mode", "Toggle Dark Mode", "Switch the system between light and dark", []string{"dark mode", "light mode", "appearance", "theme"}, core.SymbolContrast, run(core.CommandToggleDarkMode), ""},
	{"clear-clipboard", "Clear Clipboard History", "Delete every entry except pinned ones", []string{"clear history", "delete clipboard"}, core.SymbolTrash, core.ClearClipboard{}, "Delete all clipboard history except pinned entries?"},
	{"clear-calculations", "Clear Calculator History", "Delete the calculations you copied", []string{"clear calculations", "delete calculator history"}, core.SymbolTrash, core.ClearCalculations{}, "Delete all calculator history?"},
	{"plain-text", "Make Clipboard Plain Text", "Remove the formatting from the copied text", []string{"plain text", "paste as plain text", "remove formatting", "clear formatting"}, core.SymbolText, core.CopyPlainText{}, ""},
	{"settings", "System Settings", "Open the operating system settings", []string{"preferences", "control panel"}, core.SymbolSettings, run(core.CommandOpenSystemSettings), ""},
	{"tinydash-settings", "TinyDash Settings", "Shortcut, clipboard, files, and more", []string{"preferences", "configure"}, core.SymbolSettings, core.OpenSettings{}, ""},
	{"quit-tinydash", "Quit TinyDash", "Stop TinyDash until you open it again", []string{"exit"}, core.SymbolQuit, core.Quit{}, ""},
}

// Search scores every command whose name or alias matches, in list order.
func Search(m *match.Matcher, ctx Context) []core.Scored {
	var out []core.Scored
	for i := range commands {
		c := &commands[i]
		if !ctx.offers(c) {
			continue
		}
		if score, ok := m.Best(c.name, c.aliases...); ok {
			out = append(out, core.Scored{Score: score + ctx.boost(core.SourceSystem.ID(c.key)), Result: result(c, ctx)})
		}
	}
	return out
}

// Browse lists every command.
func Browse(ctx Context) []core.Result {
	var out []core.Result
	for i := range commands {
		if ctx.offers(&commands[i]) {
			out = append(out, result(&commands[i], ctx))
		}
	}
	return out
}

// Get is the result of the command with this key.
func Get(key string, ctx Context) (core.Result, bool) {
	for i := range commands {
		if commands[i].key == key && ctx.offers(&commands[i]) {
			return result(&commands[i], ctx), true
		}
	}
	return core.Result{}, false
}

func result(c *command, ctx Context) core.Result {
	id := core.SourceSystem.ID(c.key)
	pinned := ctx.pinned(id)
	pin := core.Do("Pin", core.Pin{ID: id})
	if pinned {
		pin = core.Do("Unpin", core.Unpin{ID: id})
	}
	return core.Result{
		ID:       id,
		Kind:     core.KindSystem,
		Title:    c.name,
		Subtitle: c.description,
		Icon:     core.SymbolIcon(c.symbol),
		Actions:  []core.ResultAction{{Label: c.name, Action: c.action, Confirm: c.confirm}, pin},
		Pinned:   pinned,
	}
}
