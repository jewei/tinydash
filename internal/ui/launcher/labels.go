package launcher

import "runtime"

// mac says whether keys show as macOS names them, and whether the hotkey
// recorder reads keys by position, as macOS registers them.
var mac = runtime.GOOS == "darwin"

// modKey is the app-shortcut modifier as the user knows it: ui.Cmd's key.
var modKey = map[bool]string{true: "⌘", false: "Ctrl"}[mac]

// superName is what the desktop calls the Super key: Windows calls it Win.
var superName = map[bool]string{true: "Win", false: "Super"}[runtime.GOOS == "windows"]
