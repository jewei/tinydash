package platform

import (
	"os"
	"path/filepath"
	"syscall"
)

// DiskSpace reports the volume that holds path.
func DiskSpace(path string) (Volume, error) {
	var stat syscall.Statfs_t
	if err := syscall.Statfs(path, &stat); err != nil {
		return Volume{}, err
	}
	return Volume{
		Name:       mountPoint(path),
		TotalBytes: stat.Blocks * uint64(stat.Bsize),
		FreeBytes:  stat.Bavail * uint64(stat.Bsize),
	}, nil
}

// mountPoint is the highest folder on the same device as path.
func mountPoint(path string) string {
	info, err := os.Stat(path)
	if err != nil {
		return path
	}
	device := info.Sys().(*syscall.Stat_t).Dev
	mount := path
	for dir := filepath.Dir(path); ; dir = filepath.Dir(dir) {
		parent, err := os.Stat(dir)
		if err != nil || parent.Sys().(*syscall.Stat_t).Dev != device {
			return mount
		}
		mount = dir
		if dir == filepath.Dir(dir) {
			return mount
		}
	}
}
