package platform

import (
	"os"
	"path/filepath"
)

// Dirs are the folders that hold the user's TinyDash data. They are the
// folders the Rust app used, so the Go app finds existing settings,
// history, and pins.
type Dirs struct {
	Config string // settings.json
	Data   string // tinydash.db
	Home   string
}

// AppDirs returns the folders for an app identifier, such as
// "dev.tinydash.launcher". Config is the OS config folder; Data is the
// local, not roaming, data folder, so that Windows roaming profiles never
// copy the clipboard history to a server.
func AppDirs(identifier string) (Dirs, error) {
	config, err := os.UserConfigDir()
	if err != nil {
		return Dirs{}, err
	}
	data, err := localDataDir()
	if err != nil {
		return Dirs{}, err
	}
	home, _ := os.UserHomeDir()
	return Dirs{
		Config: filepath.Join(config, identifier),
		Data:   filepath.Join(data, identifier),
		Home:   home,
	}, nil
}
