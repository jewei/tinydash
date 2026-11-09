package platform

import (
	"os"
	"path/filepath"
)

// InstalledApps reports ok false: on Windows mygo's listing serves, the
// shell's Applications folder, which also has the packaged apps that no
// Start menu shortcut names.
func InstalledApps() (apps []App, ok bool) { return nil, false }

// AppFolders are the Start menu folders, the user's and everyone's, where
// installers add an app's shortcut.
func AppFolders() []string {
	var folders []string
	for _, base := range []string{"APPDATA", "PROGRAMDATA"} {
		if dir := os.Getenv(base); dir != "" {
			folders = append(folders, filepath.Join(dir, `Microsoft\Windows\Start Menu\Programs`))
		}
	}
	return folders
}
