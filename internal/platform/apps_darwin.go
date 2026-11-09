package platform

import (
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"

	"github.com/ebitengine/purego/objc"
)

const (
	// appDepth is how many folders deep bundles are found, as the Rust app
	// found them: /Applications/Utilities/Tool.app and the apps in a
	// vendor's folder.
	appDepth = 4
	// ownBundleID is TinyDash itself, which the launcher does not list.
	ownBundleID = "dev.tinydash.launcher"
	finderPath  = "/System/Library/CoreServices/Finder.app"
)

// AppFolders are the folders apps are installed in.
func AppFolders() []string {
	folders := []string{
		"/Applications",
		"/System/Applications",
		// Keychain Access, Archive Utility, and other Apple utilities.
		"/System/Library/CoreServices/Applications",
	}
	if home, err := os.UserHomeDir(); err == nil && home != "" {
		folders = append(folders, filepath.Join(home, "Applications"))
	}
	return folders
}

// InstalledApps finds the app bundles in AppFolders, and Finder.
func InstalledApps() (apps []App, ok bool) {
	return discoverApps(AppFolders(), finderPath, readInfo), true
}

// discoverApps finds the bundles up to appDepth folders deep, without
// looking inside one, and adds finder. A symbolic link to a bundle, as
// /Applications/Safari.app is, lists the bundle at its real path, as the
// Rust app did, so pins and aliases made there stay. read reads a bundle's
// Info.plist.
func discoverApps(folders []string, finder string, read func(bundle string) map[string]any) []App {
	var apps []App
	add := func(bundle string) {
		if app, ok := bundleApp(bundle, read(bundle)); ok {
			apps = append(apps, app)
		}
	}
	for _, folder := range folders {
		// The trailing separator follows a folder that is a symbolic link,
		// as the Rust app's walk did; the paths below it keep the folder's
		// name.
		filepath.WalkDir(folder+string(filepath.Separator), func(path string, d fs.DirEntry, err error) error {
			// An unreadable folder is skipped; the walk goes on.
			if err != nil {
				return nil
			}
			rel, err := filepath.Rel(folder, path)
			if err != nil || rel == "." {
				return nil
			}
			depth := strings.Count(rel, string(filepath.Separator)) + 1
			name := d.Name()
			switch {
			case strings.HasPrefix(name, "."):
				if d.IsDir() {
					return filepath.SkipDir
				}
			case strings.HasSuffix(strings.ToLower(name), ".app"):
				if d.IsDir() {
					add(path)
					return filepath.SkipDir
				}
				if d.Type()&fs.ModeSymlink != 0 {
					if target, err := filepath.EvalSymlinks(path); err == nil {
						add(target)
					}
				}
			case d.IsDir() && depth >= appDepth:
				return filepath.SkipDir
			}
			return nil
		})
	}
	add(finder)
	return apps
}

// infoKeys are the Info.plist keys that bundleApp reads.
var infoKeys = []string{"CFBundleIdentifier", "CFBundleDisplayName", "CFBundleName", "CFBundleExecutable", "LSBackgroundOnly", "LSApplicationCategoryType"}

// bundleApp is the app of a bundle, from the values of its Info.plist
// (strings, and true or false), or false for a bundle that the launcher
// does not list: a background-only helper, or TinyDash. A bundle without
// a readable Info.plist is still an app. The bundle's file name, as Finder
// shows it, is the name; its other names and its executable are aliases,
// so `code` finds Visual Studio Code.
func bundleApp(path string, info map[string]any) (App, bool) {
	text := func(key string) string {
		s, _ := info[key].(string)
		return s
	}
	if info["LSBackgroundOnly"] == true || text("LSBackgroundOnly") == "1" || text("CFBundleIdentifier") == ownBundleID {
		return App{}, false
	}
	base := filepath.Base(path)
	app := App{Name: strings.TrimSuffix(base, filepath.Ext(base)), Path: path}
	for _, key := range []string{"CFBundleDisplayName", "CFBundleName", "CFBundleExecutable"} {
		alias := text(key)
		if alias != "" && alias != app.Name && !slices.Contains(app.Aliases, alias) {
			app.Aliases = append(app.Aliases, alias)
		}
	}
	app.Description = describe(text("CFBundleIdentifier"), text("LSApplicationCategoryType"))
	return app, true
}

// readInfo reads the infoKeys of a bundle's Info.plist, XML or binary,
// through Foundation. It is nil when the file cannot be read.
func readInfo(bundle string) map[string]any {
	if _, err := foundation(); err != nil {
		return nil
	}
	yes, err := constant("kCFBooleanTrue")
	if err != nil {
		return nil
	}
	no, err := constant("kCFBooleanFalse")
	if err != nil {
		return nil
	}
	// An autorelease pool belongs to the thread that made it.
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
	defer pool.Send(objc.RegisterName("drain"))
	url := objc.ID(objc.GetClass("NSURL")).Send(objc.RegisterName("fileURLWithPath:"), nsString(filepath.Join(bundle, "Contents", "Info.plist")))
	dictionary := objc.ID(objc.GetClass("NSDictionary")).Send(objc.RegisterName("dictionaryWithContentsOfURL:error:"), url, uintptr(0))
	if dictionary == 0 {
		return nil
	}
	stringClass := objc.GetClass("NSString")
	info := map[string]any{}
	for _, key := range infoKeys {
		value := dictionary.Send(objc.RegisterName("objectForKey:"), nsString(key))
		switch {
		case value == 0:
		case value == yes:
			info[key] = true
		case value == no:
			info[key] = false
		case objc.Send[bool](value, objc.RegisterName("isKindOfClass:"), stringClass):
			info[key] = goString(value)
		}
	}
	return info
}

// descriptions say what common apps are for, by bundle ID, so a renamed
// or localized app still gets its words. Bundles rarely describe
// themselves.
var descriptions = map[string]string{
	"com.apple.Safari":                 "Web browser",
	"com.google.Chrome":                "Web browser",
	"org.mozilla.firefox":              "Web browser",
	"com.brave.Browser":                "Web browser",
	"company.thebrowser.Browser":       "Web browser",
	"com.microsoft.edgemac":            "Web browser",
	"com.apple.finder":                 "Files and folders",
	"com.apple.Notes":                  "Notes and checklists",
	"com.apple.iCal":                   "Events and reminders",
	"com.apple.reminders":              "Tasks and reminders",
	"com.apple.mail":                   "Email",
	"com.apple.MobileSMS":              "Messages and conversations",
	"com.apple.FaceTime":               "Voice and video calls",
	"com.apple.AddressBook":            "Contacts",
	"com.apple.Preview":                "Images and PDF documents",
	"com.apple.Photos":                 "Photo library",
	"com.apple.Music":                  "Music library and player",
	"com.apple.TV":                     "Movies and TV shows",
	"com.apple.podcasts":               "Podcasts",
	"com.apple.Maps":                   "Maps and directions",
	"com.apple.weather":                "Weather forecasts",
	"com.apple.calculator":             "Calculations and conversions",
	"com.apple.TextEdit":               "Text documents",
	"com.apple.Passwords":              "Passwords and passkeys",
	"com.apple.systempreferences":      "System preferences",
	"com.apple.AppStore":               "App downloads and updates",
	"com.apple.ActivityMonitor":        "System activity",
	"com.apple.DiskUtility":            "Disk management",
	"com.apple.airport.airportutility": "Wi-Fi base station setup",
	"com.apple.audio.AudioMIDISetup":   "Audio and MIDI devices",
	"com.apple.BluetoothFileExchange":  "Send files over Bluetooth",
	"com.apple.ColorSyncUtility":       "Color profiles",
	"com.apple.Console":                "System logs",
	"com.apple.DigitalColorMeter":      "Screen color picker",
	"com.apple.MigrateAssistant":       "Move data from another Mac",
	"com.apple.printcenter":            "Printers and print jobs",
	"com.apple.ScreenSharing":          "Control another Mac",
	"com.apple.screenshot.launcher":    "Screenshots and screen recordings",
	"com.apple.ScriptEditor2":          "AppleScript editor",
	"com.apple.SystemProfiler":         "Hardware and software details",
	"com.apple.VoiceOverUtility":       "VoiceOver settings",
	"com.apple.Automator":              "Workflows",
	"com.apple.shortcuts":              "Automations",
	"com.apple.FontBook":               "Fonts",
	"com.apple.Image_Capture":          "Import photos and scans",
	"com.apple.backup.launcher":        "Backups",
	"com.apple.apps.launcher":          "All apps",
	"com.apple.siri.launcher":          "Voice assistant",
	"com.apple.exposelauncher":         "Windows and desktops",
	"com.apple.VoiceMemos":             "Voice recordings",
	"com.apple.archiveutility":         "Zip archives",
	"com.apple.keychainaccess":         "Passwords and certificates",
	"com.apple.Terminal":               "Terminal emulator",
	"com.mitchellh.ghostty":            "Terminal emulator",
	"com.googlecode.iterm2":            "Terminal emulator",
	"dev.warp.Warp-Stable":             "Terminal emulator",
	"com.apple.dt.Xcode":               "App development",
	"com.microsoft.VSCode":             "Code editor",
	"com.todesktop.230313mzl4w4u92":    "Code editor",
	"dev.zed.Zed":                      "Code editor",
	"com.google.android.studio":        "Android development",
	"com.tinyapp.TablePlus":            "Database client",
	"com.github.GitHubClient":          "Git repositories",
	"dev.kdrag0n.MacVirt":              "Containers and Linux machines",
	"com.usebruno.app":                 "API client",
	"com.spotify.client":               "Music streaming",
	"org.videolan.vlc":                 "Video player",
	"com.apple.QuickTimePlayerX":       "Video player",
	"com.larksuite.larkApp":            "Team chat and collaboration",
	"com.tinyspeck.slackmacgap":        "Team chat",
	"com.hnc.Discord":                  "Voice and text chat",
	"net.whatsapp.WhatsApp":            "Messages and calls",
	"us.zoom.xos":                      "Video meetings",
	"notion.id":                        "Notes and docs",
	"com.figma.Desktop":                "Interface design",
	"com.anthropic.claudefordesktop":   "AI assistant",
	"com.apple.iWork.Pages":            "Documents",
	"com.apple.iWork.Numbers":          "Spreadsheets",
	"com.apple.iWork.Keynote":          "Presentations",
}

// categories name the App Store categories of LSApplicationCategoryType,
// without its "public.app-category." prefix.
var categories = map[string]string{
	"business":           "Business",
	"developer-tools":    "Developer tools",
	"education":          "Education",
	"entertainment":      "Entertainment",
	"finance":            "Finance",
	"graphics-design":    "Graphics and design",
	"healthcare-fitness": "Health and fitness",
	"lifestyle":          "Lifestyle",
	"medical":            "Medical",
	"music":              "Music",
	"news":               "News",
	"photography":        "Photography",
	"productivity":       "Productivity",
	"reference":          "Reference",
	"social-networking":  "Social networking",
	"sports":             "Sports",
	"travel":             "Travel",
	"utilities":          "Utility",
	"video":              "Video",
	"weather":            "Weather",
	"books":              "Books",
	"games":              "Game",
	"action-games":       "Game",
	"adventure-games":    "Game",
	"arcade-games":       "Game",
	"board-games":        "Game",
	"card-games":         "Game",
	"casino-games":       "Game",
	"dice-games":         "Game",
	"educational-games":  "Game",
	"family-games":       "Game",
	"kids-games":         "Game",
	"music-games":        "Game",
	"puzzle-games":       "Game",
	"racing-games":       "Game",
	"role-playing-games": "Game",
	"simulation-games":   "Game",
	"sports-games":       "Game",
	"strategy-games":     "Game",
	"trivia-games":       "Game",
	"word-games":         "Game",
}

// describe gives an app's words: its entry in descriptions, else its App
// Store category, else none.
func describe(bundleID, category string) string {
	if words, ok := descriptions[bundleID]; ok {
		return words
	}
	kind, ok := strings.CutPrefix(category, "public.app-category.")
	if !ok {
		return ""
	}
	return categories[kind]
}
