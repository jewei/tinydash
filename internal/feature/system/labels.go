package system

import "runtime"

var (
	emptyTrash = pick("Empty Trash", "Empty Recycle Bin", "Empty Trash")
	openTrash  = pick("Open Trash", "Open Recycle Bin", "Open Trash")
	// sleepDisplays: macOS sleeps displays; the others turn them off.
	sleepDisplays = pick("Sleep Displays", "Turn Off Displays", "Turn Off Displays")
	// On Linux desktops without a known session manager, systemd restarts
	// and shuts down without asking apps to quit, so the text promises
	// nothing.
	confirmRestart  = pick("Restart the computer? Apps will be asked to quit.", "Restart the computer? Apps will be asked to quit.", "Restart the computer? Save your work first.")
	confirmShutDown = pick("Shut down the computer? Apps will be asked to quit.", "Shut down the computer? Apps will be asked to quit.", "Shut down the computer? Save your work first.")
)

// pick chooses the label for this OS: macOS, Windows, or Linux and others.
func pick(macOS, windows, linux string) string {
	switch runtime.GOOS {
	case "darwin":
		return macOS
	case "windows":
		return windows
	}
	return linux
}
