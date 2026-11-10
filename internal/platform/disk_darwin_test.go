package platform

import (
	"os"
	"syscall"
	"testing"
)

// Finder counts purgeable space, so DiskSpace reports at least what statfs
// calls available.
func TestDiskSpaceCountsPurgeableSpace(t *testing.T) {
	home, _ := os.UserHomeDir()
	v, err := DiskSpace(home)
	if err != nil {
		t.Fatal(err)
	}
	var stat syscall.Statfs_t
	if err := syscall.Statfs(home, &stat); err != nil {
		t.Fatal(err)
	}
	if statfsFree := stat.Bavail * uint64(stat.Bsize); v.FreeBytes < statfsFree*99/100 {
		t.Errorf("free %d bytes, statfs says %d are available", v.FreeBytes, statfsFree)
	}
	if _, err := DiskSpace("relative"); err == nil {
		t.Error("a relative path should fail")
	}
}
