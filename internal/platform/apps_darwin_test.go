package platform

import (
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"slices"
	"strings"
	"testing"
)

func TestDescribesKnownAppsThenByCategory(t *testing.T) {
	for _, c := range []struct{ id, category, want string }{
		{"com.apple.Safari", "public.app-category.productivity", "Web browser"},
		{"eu.exelban.Stats", "public.app-category.utilities", "Utility"},
		{"com.apple.Chess", "public.app-category.board-games", "Game"},
		{"com.example", "public.app-category.new-kind", ""},
		{"com.example", "public.app-category.new-games", ""},
		{"com.example", "utilities", ""},
		{"", "", ""},
	} {
		if got := describe(c.id, c.category); got != c.want {
			t.Errorf("describe(%q, %q) = %q, want %q", c.id, c.category, got, c.want)
		}
	}
}

func TestABundleIsNamedByItsFileWithItsOtherNamesAsAliases(t *testing.T) {
	for _, c := range []struct {
		name string
		path string
		info map[string]any
		want *App
	}{
		{"no Info.plist", "/Applications/Broken.app", nil, &App{Name: "Broken", Path: "/Applications/Broken.app"}},
		{"bundle names and the executable", "/Applications/Visual Studio Code.app",
			map[string]any{"CFBundleIdentifier": "com.microsoft.VSCode", "CFBundleDisplayName": "Code", "CFBundleName": "Code", "CFBundleExecutable": "Electron"},
			&App{Name: "Visual Studio Code", Path: "/Applications/Visual Studio Code.app", Aliases: []string{"Code", "Electron"}, Description: "Code editor"}},
		{"names that are the file name", "/Applications/Slack.app",
			map[string]any{"CFBundleName": "Slack", "CFBundleExecutable": "Slack", "CFBundleDisplayName": ""},
			&App{Name: "Slack", Path: "/Applications/Slack.app"}},
		{"a dot in the name", "/Applications/Foo.Bar.app", nil, &App{Name: "Foo.Bar", Path: "/Applications/Foo.Bar.app"}},
		{"category", "/Applications/Stats.app", map[string]any{"LSApplicationCategoryType": "public.app-category.utilities"},
			&App{Name: "Stats", Path: "/Applications/Stats.app", Description: "Utility"}},
		{"background only", "/Applications/Helper.app", map[string]any{"LSBackgroundOnly": true}, nil},
		{"background only as a string", "/Applications/Helper.app", map[string]any{"LSBackgroundOnly": "1"}, nil},
		{"background off", "/Applications/Helper.app", map[string]any{"LSBackgroundOnly": false}, &App{Name: "Helper", Path: "/Applications/Helper.app"}},
		{"TinyDash itself", "/Applications/TinyDash.app", map[string]any{"CFBundleIdentifier": "dev.tinydash.launcher"}, nil},
	} {
		got, ok := bundleApp(c.path, c.info)
		if c.want == nil {
			if ok {
				t.Errorf("%s: listed as %+v", c.name, got)
			}
			continue
		}
		if !ok || !reflect.DeepEqual(got, *c.want) {
			t.Errorf("%s: got %+v (%v), want %+v", c.name, got, ok, *c.want)
		}
	}
}

// writeBundle makes a bundle whose Info.plist holds info, or none when
// info is empty; binary saves the plist in the binary format.
func writeBundle(t *testing.T, path, info string, binary bool) {
	t.Helper()
	contents := filepath.Join(path, "Contents")
	if err := os.MkdirAll(contents, 0o755); err != nil {
		t.Fatal(err)
	}
	if info == "" {
		return
	}
	plist := filepath.Join(contents, "Info.plist")
	text := `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>` + info + `</dict></plist>`
	if err := os.WriteFile(plist, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	if binary {
		if out, err := exec.Command("plutil", "-convert", "binary1", plist).CombinedOutput(); err != nil {
			t.Fatalf("plutil: %v: %s", err, out)
		}
	}
}

func id(bundleID string) string {
	return "<key>CFBundleIdentifier</key><string>" + bundleID + "</string>"
}

// The bundles of a fixture folder become the apps of the search golden
// world (internal/search/search_golden_test.go), aliases and descriptions
// included, with the rules of the Rust app's discovery.
func TestDiscoversBundlesAsTheRustAppDid(t *testing.T) {
	root, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	apps := filepath.Join(root, "Applications")
	system := filepath.Join(root, "System", "Applications")
	cryptex := filepath.Join(root, "Cryptexes", "Safari.app")
	writeBundle(t, cryptex, id("com.apple.Safari")+"<key>CFBundleName</key><string>Safari</string>", true)
	if err := os.MkdirAll(apps, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(cryptex, filepath.Join(apps, "Safari.app")); err != nil {
		t.Fatal(err)
	}
	writeBundle(t, filepath.Join(apps, "Visual Studio Code.app"), id("com.microsoft.VSCode")+"<key>CFBundleExecutable</key><string>code</string>", false)
	writeBundle(t, filepath.Join(apps, "Slack.app"), id("com.tinyspeck.slackmacgap")+"<key>CFBundleName</key><string>Slack</string>", true)
	writeBundle(t, filepath.Join(apps, "Vendor", "Suite", "Tools", "Deep.app"), "", false)
	writeBundle(t, filepath.Join(apps, "a", "b", "c", "d", "TooDeep.app"), "", false)
	writeBundle(t, filepath.Join(apps, "Helper.app"), "<key>LSBackgroundOnly</key><true/>", true)
	writeBundle(t, filepath.Join(apps, "OldHelper.app"), "<key>LSBackgroundOnly</key><string>1</string>", false)
	writeBundle(t, filepath.Join(apps, "TinyDash.app"), id("dev.tinydash.launcher"), false)
	writeBundle(t, filepath.Join(apps, ".Hidden.app"), "", false)
	writeBundle(t, filepath.Join(apps, ".cache", "Cached.app"), "", false)
	writeBundle(t, filepath.Join(apps, "Outer.app", "Contents", "Inner.app"), "", false)
	writeBundle(t, filepath.Join(system, "Utilities", "Terminal.app"), "<key>LSApplicationCategoryType</key><string>public.app-category.board-games</string>", false)
	writeBundle(t, filepath.Join(root, "Finder.app"), id("com.apple.finder"), false)

	got := discoverApps([]string{apps, system, filepath.Join(root, "Missing")}, filepath.Join(root, "Finder.app"), readInfo)
	want := []App{
		{Name: "Outer", Path: filepath.Join(apps, "Outer.app")},
		{Name: "Safari", Path: cryptex, Description: "Web browser"},
		{Name: "Slack", Path: filepath.Join(apps, "Slack.app"), Description: "Team chat"},
		{Name: "Deep", Path: filepath.Join(apps, "Vendor", "Suite", "Tools", "Deep.app")},
		{Name: "Visual Studio Code", Path: filepath.Join(apps, "Visual Studio Code.app"), Aliases: []string{"code"}, Description: "Code editor"},
		{Name: "Terminal", Path: filepath.Join(system, "Utilities", "Terminal.app"), Description: "Game"},
		{Name: "Finder", Path: filepath.Join(root, "Finder.app"), Description: "Files and folders"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("found\n%+v\nwant\n%+v", got, want)
	}
}

func TestFindsSystemAppsAndNothingInsideABundle(t *testing.T) {
	found, ok := InstalledApps()
	if !ok {
		t.Fatal("macOS has its own discovery")
	}
	names := map[string]bool{}
	for _, app := range found {
		names[app.Name] = true
		if strings.Contains(app.Path, "/Contents/") {
			t.Errorf("listed %s, inside a bundle", app.Path)
		}
	}
	for _, name := range []string{"Finder", "Archive Utility"} {
		if !names[name] {
			t.Errorf("%s is missing", name)
		}
	}
	finder := slices.IndexFunc(found, func(app App) bool { return app.Name == "Finder" })
	if finder >= 0 && found[finder].Description != "Files and folders" {
		t.Errorf("Finder says %q", found[finder].Description)
	}
}
