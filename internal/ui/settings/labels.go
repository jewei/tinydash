package settings

import "runtime"

// mac says whether keys show as macOS names them, whether the recorder
// reads keys by position, as macOS registers them, and whether Spotlight
// can search files. The model keeps a copy, which tests set.
var mac = runtime.GOOS == "darwin"

// richClipboard: Linux saves copied text only, so it has no image or file
// history to turn on.
var richClipboard = runtime.GOOS != "linux"

// trayName is what the desktop calls the tray.
var trayName = map[bool]string{true: "Show menu bar icon", false: "Show tray icon"}[runtime.GOOS == "darwin"]
