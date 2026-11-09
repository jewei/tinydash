package settings

import "runtime"

// macOS users reach TinyDash from the shortcut; the menu bar is crowded.
var defaultShowTrayIcon = runtime.GOOS != "darwin"
