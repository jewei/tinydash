package platform

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestTheHiddenFlagHidesAFile(t *testing.T) {
	dir := t.TempDir()
	shown, hidden := filepath.Join(dir, "shown"), filepath.Join(dir, "hidden")
	for _, p := range []string{shown, hidden} {
		if err := os.WriteFile(p, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := syscall.Chflags(hidden, 0x8000); err != nil {
		t.Skip(err)
	}
	check := HiddenCheck()
	if check(shown) || !check(hidden) {
		t.Fatalf("shown %v, hidden %v", check(shown), check(hidden))
	}
}
