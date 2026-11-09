package app

import (
	"github.com/egoist/mygo"

	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/platform"
)

// installedApps lists the installed apps. On macOS and Linux it uses the
// Rust app's discovery, which keeps the Rust app's paths (so pins, usage,
// aliases, and hotkeys stay) and finds the aliases and descriptions. On
// Windows it uses mygo's listing of the shell's Applications folder.
// mygo's Shell.LaunchApp uses only the path, so every listed app launches.
func installedApps() ([]apps.App, error) {
	if found, ok := platform.InstalledApps(); ok {
		list := make([]apps.App, len(found))
		for i, app := range found {
			list[i] = apps.App(app)
		}
		return list, nil
	}
	found, err := mygo.Shell.InstalledApps()
	if err != nil {
		return nil, err
	}
	list := make([]apps.App, len(found))
	for i, app := range found {
		list[i] = apps.App{Name: app.Name, Path: app.Path}
	}
	return list, nil
}
