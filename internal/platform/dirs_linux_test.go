package platform

import (
	"path/filepath"
	"testing"
)

func TestLinuxFoldersDefaultToTheXDGPlaces(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", "")
	t.Setenv("XDG_DATA_HOME", "")
	t.Setenv("HOME", "/home/me")
	d, err := AppDirs("dev.tinydash.launcher")
	if err != nil {
		t.Fatal(err)
	}
	if d.Config != "/home/me/.config/dev.tinydash.launcher" || d.Data != "/home/me/.local/share/dev.tinydash.launcher" {
		t.Fatalf("%+v", d)
	}
}

func TestARelativeConfigHomeIsRefused(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", "relative")
	t.Setenv("HOME", "/home/me")
	if _, err := AppDirs("dev.tinydash.launcher"); err == nil {
		t.Fatal("a relative XDG_CONFIG_HOME was accepted")
	}
}

func TestTheFoldersFollowXDG(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", "/x/config")
	t.Setenv("XDG_DATA_HOME", "/x/data")
	d, err := AppDirs("dev.tinydash.launcher")
	if err != nil {
		t.Fatal(err)
	}
	if d.Config != filepath.Join("/x/config", "dev.tinydash.launcher") || d.Data != filepath.Join("/x/data", "dev.tinydash.launcher") {
		t.Fatalf("%+v", d)
	}
	t.Setenv("XDG_DATA_HOME", "relative")
	t.Setenv("HOME", "/home/me")
	if d, _ := AppDirs("x"); d.Data != "/home/me/.local/share/x" {
		t.Fatalf("relative XDG_DATA_HOME: %+v", d)
	}
}
