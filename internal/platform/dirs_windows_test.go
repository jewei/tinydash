package platform

import (
	"path/filepath"
	"testing"
)

// Settings roam with the profile; the clipboard history stays local.
func TestDataIsLocalNotRoaming(t *testing.T) {
	t.Setenv("AppData", `C:\Users\me\AppData\Roaming`)
	t.Setenv("LocalAppData", `C:\Users\me\AppData\Local`)
	d, err := AppDirs("dev.tinydash.launcher")
	if err != nil {
		t.Fatal(err)
	}
	if d.Config != filepath.Join(`C:\Users\me\AppData\Roaming`, "dev.tinydash.launcher") || d.Data != filepath.Join(`C:\Users\me\AppData\Local`, "dev.tinydash.launcher") {
		t.Fatalf("%+v", d)
	}
}
