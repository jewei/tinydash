package platform

import (
	"os"
	"testing"
)

func TestDiskSpaceOfTheHomeFolder(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Skip(err)
	}
	v, err := DiskSpace(home)
	if err != nil {
		t.Fatal(err)
	}
	if v.Name == "" || v.TotalBytes == 0 || v.FreeBytes > v.TotalBytes {
		t.Fatalf("implausible volume %+v", v)
	}
	t.Logf("%s: %.1f GB free of %.1f GB", v.Name, float64(v.FreeBytes)/1e9, float64(v.TotalBytes)/1e9)
}
