package platform

import (
	"errors"
	"os"
)

// localDataDir is %LOCALAPPDATA%, not the roaming %APPDATA%.
func localDataDir() (string, error) {
	if dir := os.Getenv("LocalAppData"); dir != "" {
		return dir, nil
	}
	return "", errors.New("Windows did not say where local app data goes (%LocalAppData% is not set).")
}
