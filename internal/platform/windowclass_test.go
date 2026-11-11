package platform

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"testing"
)

// The desktop file mygo writes sets StartupWMClass to the slug, the name
// of the Linux executable. The window class has to be that same string.
func TestDesktopWindowClassIsTheLinuxExecutable(t *testing.T) {
	var cfg struct {
		Slug string `json:"slug"`
	}
	b, err := os.ReadFile(filepath.Join("..", "..", "mygo.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(b, &cfg); err != nil {
		t.Fatal(err)
	}
	if cfg.Slug != DesktopWindowClass {
		t.Fatalf("StartupWMClass follows the slug %q, and the window class is %q", cfg.Slug, DesktopWindowClass)
	}
}

// Native windows never load WebKitGTK, and the tray needs AppIndicator.
// mygo's package lists WebKit only while linux.webview is true, and adds
// linux.depends after GTK.
func TestTheLinuxPackageDependsOnGTKAndTheTray(t *testing.T) {
	var cfg struct {
		Linux struct {
			WebView *bool    `json:"webview"`
			Depends []string `json:"depends"`
		} `json:"linux"`
	}
	b, err := os.ReadFile(filepath.Join("..", "..", "mygo.json"))
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(b, &cfg); err != nil {
		t.Fatal(err)
	}
	if cfg.Linux.WebView == nil || *cfg.Linux.WebView {
		t.Fatal("linux.webview is not false, so the package would depend on WebKitGTK")
	}
	if !slices.Contains(cfg.Linux.Depends, "libayatana-appindicator3-1") {
		t.Fatalf("depends %q, want libayatana-appindicator3-1", cfg.Linux.Depends)
	}
	for _, dep := range cfg.Linux.Depends {
		if strings.Contains(strings.ToLower(dep), "webkit") {
			t.Fatalf("depends on WebKitGTK: %s", dep)
		}
	}
}

func TestAlignWindowClassIsANoOpOffLinux(t *testing.T) {
	if runtime.GOOS == "linux" {
		t.Skip("Linux sets the class")
	}
	if err := AlignWindowClass(); err != nil {
		t.Fatal(err)
	}
}
