package files

import "runtime"

// fileManager names the OS file manager in "Show in …".
var fileManager = func() string {
	switch runtime.GOOS {
	case "darwin":
		return "Finder"
	case "windows":
		return "File Explorer"
	}
	return "Files"
}()

// nativeIcons: only macOS gives file icons as images; elsewhere a symbol
// shows.
var nativeIcons = runtime.GOOS == "darwin"
