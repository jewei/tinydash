package app

import (
	"io/fs"
	"os"
	"path/filepath"
)

// osDisk is the real disk for the file index.
type osDisk struct{}

func (osDisk) ReadDir(dir string) ([]fs.DirEntry, error) { return os.ReadDir(dir) }

func (osDisk) RealDir(dir string) (string, bool) {
	real, err := filepath.EvalSymlinks(dir)
	if err != nil {
		return dir, false
	}
	info, err := os.Stat(real)
	return real, err == nil && info.IsDir()
}
