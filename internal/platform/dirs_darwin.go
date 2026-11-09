package platform

import (
	"errors"
	"os"
	"path/filepath"
)

// localDataDir is ~/Library/Application Support, as for config.
func localDataDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil || home == "" {
		return "", errNoHome
	}
	return filepath.Join(home, "Library", "Application Support"), nil
}

var errNoHome = errors.New("TinyDash could not find your home folder.")
