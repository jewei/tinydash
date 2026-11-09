package launcher

import (
	"slices"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
)

// The tests below port src/launcher/PreviewPane.tsx and its place in
// Launcher.tsx.

func hasDetails(tt *ui.Tester) bool {
	_, ok := tt.AccessByRole(ui.RoleRegion, "Details")
	return ok
}

var safariFacts = Details{Details: engine.Details{Kind: engine.DetailsFile, File: engine.FileFacts{
	Path: safariPath, Size: 4_200_000, Modified: time.Date(2026, 1, 5, 13, 7, 9, 0, time.Local), IsDir: false,
}}}

func TestShowsTheDetailsOfTheSelectedResult(t *testing.T) {
	notes := app("Notes")
	f := newFake(func(q string, _ core.Category) []core.Result {
		if q != "" {
			return []core.Result{safari, notes, restart}
		}
		return nil
	})
	f.details = map[core.ID]Details{safari.ID: safariFacts}
	_, tt := open(t, f)
	tt.Type("a")
	if !hasDetails(tt) {
		t.Fatalf("no details pane; texts %q", tt.Texts())
	}
	for _, text := range []string{"APPLICATION", "Where", safariPath, "Size", "4.2 MB", "Modified", "Show in Finder"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	tt.Key(0, ui.KeyDown)
	// Notes has no details to load; its facts must not show Safari's.
	if tt.HasText(safariPath) || !slices.Equal(f.detailLoads, []core.ID{safari.ID, notes.ID}) {
		t.Errorf("loads %v; texts %q", f.detailLoads, tt.Texts())
	}
	// A command has nothing to load: its row's words and actions show.
	tt.Key(0, ui.KeyDown)
	if len(f.detailLoads) != 2 || !tt.HasText("COMMAND") || !tt.HasText("Run") {
		t.Errorf("loads %v; texts %q", f.detailLoads, tt.Texts())
	}
}

// A slow load that ends after the user moved on is dropped: the latest
// selection wins.
func TestTheLatestSelectionsDetailsWin(t *testing.T) {
	notes := app("Notes")
	f := newFake(always(safari, notes))
	f.details = map[core.ID]Details{
		safari.ID: safariFacts,
		notes.ID:  {Details: engine.Details{Kind: engine.DetailsFile, File: engine.FileFacts{Path: "notes-path", IsDir: true}}},
	}
	f.holdDetails = true
	_, tt := open(t, f)
	tt.Frame()
	tt.Key(0, ui.KeyDown)
	f.detailsWaiting[1]()
	f.detailsWaiting[0]()
	tt.Frame()
	if !tt.HasText("notes-path") || tt.HasText(safariPath) || tt.HasText("Size") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestShowsWidgetsInPlaceOfTheDetailsOnAnEmptyAllSearch(t *testing.T) {
	f := newFake(always(safari))
	f.widgets = Widgets{Clocks: true}
	_, tt := showLauncher(t, f)
	if hasDetails(tt) || !hasPane(tt) {
		t.Fatalf("details %v, widgets %v", hasDetails(tt), hasPane(tt))
	}
	tt.Type("s")
	if !hasDetails(tt) || hasPane(tt) {
		t.Errorf("after typing: details %v, widgets %v", hasDetails(tt), hasPane(tt))
	}
	// With every widget off, the details stay on the empty All search.
	f.widgets = Widgets{}
	_, tt = showLauncher(t, f)
	if !hasDetails(tt) {
		t.Error("no details with every widget off")
	}
}

func TestShowsAClipboardTextOnceAndRunsActionsFromThePane(t *testing.T) {
	long := clip
	long.Title = "First line"
	f := newFake(always(long))
	f.details = map[core.ID]Details{long.ID: {Details: engine.Details{Kind: engine.DetailsText, Text: "First line\nSecond line"}}}
	_, tt := open(t, f)
	tt.Type("f")
	if !tt.HasText("First line\nSecond line") {
		t.Fatalf("texts %q", tt.Texts())
	}
	// The text says the title; the pane does not say it again.
	texts := tt.Texts()
	if n := count(texts[slices.Index(texts, "Details"):], "First line"); n != 0 {
		t.Errorf("the title shows %d times; texts %q", n, tt.Texts())
	}
	pressButton(t, tt, "Pin")
	wantRuns(t, f, ran{long.Actions[1].Action, ""})
}

func TestLoadsTheDetailsAgainWhenTheDataChanged(t *testing.T) {
	f := newFake(always(safari))
	f.details = map[core.ID]Details{safari.ID: safariFacts}
	m, tt := open(t, f)
	tt.Type("s")
	loads := len(f.detailLoads)
	m.Stale()
	tt.Frame()
	if len(f.detailLoads) != loads+1 {
		t.Errorf("loads %v", f.detailLoads)
	}
}

func count(texts []string, s string) int {
	n := 0
	for _, t := range texts {
		if t == s {
			n++
		}
	}
	return n
}
