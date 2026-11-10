package settings

import (
	"slices"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestAboutShowsTheVersionAndFolders(t *testing.T) {
	f := newFake()
	f.about = About{Version: "0.10.0", Settings: "/config/tinydash", Data: "/data/tinydash"}
	_, tt := open(t, f, about)
	if _, ok := tt.AccessByRole(ui.RoleHeading, "TinyDash 0.10.0"); !ok {
		t.Errorf("no heading with the version; texts %q", tt.Texts())
	}
	// Each folder is a group named by its label, so a screen reader reads
	// the label with the path.
	for label, path := range map[string]string{"Settings": "/config/tinydash", "Saved data": "/data/tinydash"} {
		if _, ok := tt.AccessByRole(ui.RoleGroup, label); !ok {
			t.Errorf("no group %q", label)
		}
		if !slices.Contains(tt.Texts(), path) {
			t.Errorf("About lacks %q; texts %q", path, tt.Texts())
		}
	}
	if _, ok := tt.AccessByRole(ui.RoleList, "Credits"); !ok {
		t.Errorf("no list of credits")
	}
}

// With --data-dir, settings.json and tinydash.db share one folder, shown
// once; a run from source has no version.
func TestAboutFromSourceWithOneFolder(t *testing.T) {
	f := newFake()
	f.about = About{Settings: "/scratch", Data: "/scratch"}
	_, tt := open(t, f, about)
	texts := tt.Texts()
	for _, want := range []string{"TinyDash, run from source", "Settings and saved data", "/scratch"} {
		if !slices.Contains(texts, want) {
			t.Errorf("About lacks %q; texts %q", want, texts)
		}
	}
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Saved data"); ok {
		t.Errorf("one folder shown twice; texts %q", texts)
	}
}

// A path with no spaces, as on Windows, breaks inside the window at its
// narrowest instead of running past the edge.
func TestAboutLongPathFitsTheNarrowestWindow(t *testing.T) {
	const width = 640
	path := `C:\Users\Christopher.Abernathy\AppData\Local\dev.tinydash.launcher\data\folder`
	f := newFake()
	f.about = About{Version: "0.10.0", Settings: path, Data: path}
	m := New(f)
	tt := ui.NewTester(m.View, width, 440)
	click(t, tt, "About")
	text, ok := tt.Access(path)
	if !ok {
		t.Fatalf("no path text; texts %q", tt.Texts())
	}
	if right := text.Bounds.X + text.Bounds.W; right > width {
		t.Errorf("the path runs to x=%v, past the window's %v", right, width)
	}
	if !slices.Contains(tt.Texts(), path) {
		t.Errorf("the path is not shown whole; texts %q", tt.Texts())
	}
}
