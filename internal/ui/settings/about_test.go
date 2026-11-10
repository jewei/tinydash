package settings

import (
	"slices"
	"testing"
)

func TestAboutShowsTheVersionAndFolders(t *testing.T) {
	f := newFake()
	f.about = About{Version: "0.10.0", Settings: "/config/tinydash", Data: "/data/tinydash"}
	_, tt := open(t, f, about)
	texts := tt.Texts()
	for _, want := range []string{"TinyDash 0.10.0", "Settings", "/config/tinydash", "Saved data", "/data/tinydash", "Credits"} {
		if !slices.Contains(texts, want) {
			t.Errorf("About lacks %q; texts %q", want, texts)
		}
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
	if slices.Contains(texts, "Saved data") {
		t.Errorf("one folder shown twice; texts %q", texts)
	}
}
