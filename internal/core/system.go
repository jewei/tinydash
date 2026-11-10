package core

// SystemCommand is an OS command from the System tab. The values are the
// Rust app's names; internal/platform runs each one on every OS.
type SystemCommand string

const (
	CommandLock               SystemCommand = "lock"
	CommandSleep              SystemCommand = "sleep"
	CommandRestart            SystemCommand = "restart"
	CommandShutDown           SystemCommand = "shutDown"
	CommandLogOut             SystemCommand = "logOut"
	CommandEmptyTrash         SystemCommand = "emptyTrash"
	CommandOpenSystemSettings SystemCommand = "openSystemSettings"
	CommandSleepDisplays      SystemCommand = "sleepDisplays"
	CommandToggleDarkMode     SystemCommand = "toggleDarkMode"
	CommandOpenTrash          SystemCommand = "openTrash"
)

// SystemCommands lists every OS command.
func SystemCommands() []SystemCommand {
	return []SystemCommand{CommandLock, CommandSleep, CommandRestart, CommandShutDown, CommandLogOut, CommandEmptyTrash, CommandOpenSystemSettings, CommandSleepDisplays, CommandToggleDarkMode, CommandOpenTrash}
}
