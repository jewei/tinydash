package platform

import (
	"os"
	"path/filepath"
)

// localDataDir is $XDG_DATA_HOME, or ~/.local/share.
func localDataDir() (string, error) {
	if dir := os.Getenv("XDG_DATA_HOME"); filepath.IsAbs(dir) {
		return dir, nil
	}
	home, err := os.UserHomeDir()
	if err != nil || home == "" {
		return "", errNoHome
	}
	return filepath.Join(home, ".local", "share"), nil
}
