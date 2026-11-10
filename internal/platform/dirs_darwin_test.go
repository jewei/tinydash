package platform

import (
	"os"
	"path/filepath"
	"testing"
)

// The Rust app kept both files in Application Support under its identifier.
func TestTheFoldersAreTheRustApps(t *testing.T) {
	home, _ := os.UserHomeDir()
	d, err := AppDirs("dev.tinydash.launcher")
	if err != nil {
		t.Fatal(err)
	}
	want := filepath.Join(home, "Library", "Application Support", "dev.tinydash.launcher")
	if d.Config != want || d.Data != want || d.Home != home {
		t.Fatalf("%+v, want config and data %s", d, want)
	}
}
