package search

import (
	"encoding/json"
	"fmt"
	"io/fs"
	"math/rand/v2"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"testing/fstest"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/calc"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/emoji"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/feature/system"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/match"
	"github.com/jewei/tinydash/internal/settings"
)

// mapDisk serves a MapFS as a files.Disk, with paths as the MapFS names
// them, so a root can be the literal "<root>" of the corpus.
type mapDisk fstest.MapFS

func (d mapDisk) ReadDir(dir string) ([]fs.DirEntry, error) {
	return fs.ReadDir(fstest.MapFS(d), dir)
}

func (d mapDisk) RealDir(dir string) (string, bool) {
	info, err := fs.Stat(fstest.MapFS(d), dir)
	return dir, err == nil && info.IsDir()
}

// goldenWorld is the fixed world of golden_search in src-tauri/src/golden.rs.
func goldenWorld(t testing.TB, now time.Time) *Snapshot {
	disk := mapDisk{}
	for _, file := range []string{
		"Documents/report.pdf", "Documents/Budget 2026.xlsx", "Documents/notes.txt",
		"Projects/tinydash/README.md", "Projects/tinydash/node_modules/left-pad/index.js",
		"Projects/notes-app/main.go", ".hidden/secret.txt", "Photos/beach.png", "Photos/Report card.jpg",
	} {
		disk["<root>/"+file] = &fstest.MapFile{Data: []byte("x")}
	}
	const day = 86_400
	unix := now.Unix()
	snippet := func(id int64, kind library.Kind, name, keyword, text string) library.Item {
		return library.Item{ID: id, Kind: kind, Name: name, Keyword: keyword, Text: text}
	}
	return &Snapshot{
		Settings: settings.Default(),
		Apps: apps.NewIndex([]apps.App{
			{Name: "Safari", Path: "/Applications/Safari.app", Description: "Web browser"},
			{Name: "Visual Studio Code", Path: "/Applications/Visual Studio Code.app", Aliases: []string{"code"}, Description: "Code editor"},
			{Name: "Terminal", Path: "/System/Applications/Utilities/Terminal.app"},
			{Name: "Notes", Path: "/System/Applications/Notes.app", Description: "Notes"},
			{Name: "Chess", Path: "/System/Applications/Chess.app"},
			{Name: "Calculator", Path: "/System/Applications/Calculator.app"},
			{Name: "System Settings", Path: "/System/Applications/System Settings.app"},
			{Name: "Slack", Path: "/Applications/Slack.app", Description: "Team chat"},
		}),
		Files: files.Scan(disk, files.Options{Roots: []string{"<root>"}, Excluded: []string{"node_modules"}, Packages: []string{"app"}}),
		Clipboard: clipboard.NewHistory([]clipboard.Entry{
			{ID: 3, Kind: clipboard.KindText, Title: "hello world", Text: "hello world", CopiedAt: unix - 60},
			{ID: 2, Kind: clipboard.KindFiles, Title: "report.pdf", Text: "/tmp/report.pdf", CopiedAt: unix - 3600},
			{ID: 1, Kind: clipboard.KindText, Title: "ssh deploy@example.com", Text: "ssh deploy@example.com", CopiedAt: unix - day},
		}),
		Library: library.NewIndex([]library.Item{
			snippet(1, library.KindSnippet, "Signature", "sig", "Best,\nJ"),
			snippet(2, library.KindSnippet, "Address", "addr", "1 Main St"),
			snippet(3, library.KindQuicklink, "Search MDN", "mdn", "https://developer.mozilla.org/search?q={query}"),
			snippet(4, library.KindQuicklink, "Tinydash repo", "", "https://github.com/jewei/tinydash"),
		}),
		Calculations: calc.NewHistory(nil),
		Emoji:        emoji.NewIndex(),
		Usage: NewUsage(map[core.ID]core.Use{
			"app:/Applications/Slack.app":        {Count: 12, LastUsed: unix - 3600},
			"app:/System/Applications/Notes.app": {Count: 3, LastUsed: unix - 10*day},
			"emoji:🚀":                            {Count: 4, LastUsed: unix - day},
			"snippet:1":                          {Count: 2, LastUsed: unix - 2*day},
			"system:lock":                        {Count: 5, LastUsed: unix},
		}),
		Pins:       NewPins([]core.ID{"app:/Applications/Safari.app", "clip:2", "emoji:🎉"}),
		Hidden:     NewHidden("app:/System/Applications/Chess.app"),
		Aliases:    NewAliases(map[core.ID]string{"app:/Applications/Visual Studio Code.app": "vsc"}),
		Hotkeys:    NewHotkeys(map[core.ID]string{"snippet:1": "Control+Alt+S"}),
		Now:        now,
		Home:       "/Users/golden",
		Random:     rand.NewChaCha8([32]byte{1}),
		AppsReady:  true,
		FilesReady: true,
	}
}

// reorderTies puts got in the order of want (result IDs) where the two
// differ only among equal scores: at each position where they differ, Go
// scores both entries the same. Rust's order of ties, and which of them a
// source's limit cut, came from an unstable selection; Go keeps ties in
// input order. hitOf finds a hit that Go ranks but cut, with its score.
func reorderTies(want []core.ID, got []core.Scored, hitOf func(core.ID) (core.Scored, bool)) ([]core.Scored, error) {
	if len(want) != len(got) {
		return nil, fmt.Errorf("%d results, Rust had %d", len(got), len(want))
	}
	byID := map[core.ID]core.Scored{}
	for _, hit := range got {
		byID[hit.Result.ID] = hit
	}
	out := make([]core.Scored, len(want))
	for i, id := range want {
		hit, ok := byID[id]
		if !ok {
			hit, ok = hitOf(id)
		}
		if !ok {
			return nil, fmt.Errorf("Rust has %s at %d; Go does not find it", id, i)
		}
		if id != got[i].Result.ID && (hit.Score == unscored || hit.Score != got[i].Score) {
			return nil, fmt.Errorf("Rust has %s at %d, Go has %s, and Go scores them %d and %d", id, i, got[i].Result.ID, hit.Score, got[i].Score)
		}
		out[i] = hit
	}
	return out, nil
}

// sourceHit is the hit of id in its source's search for query, with no
// limit, as search shows it.
func sourceHit(s *Snapshot, query string, id core.ID) (core.Scored, bool) {
	v := newView(s)
	m := match.New(strings.TrimSpace(query))
	const everything = 1 << 30
	source, _, ok := core.ParseID(id)
	if !ok {
		return core.Scored{}, false
	}
	var hits []core.Scored
	switch source {
	case core.SourceApp:
		hits = v.Apps.Search(m, v.apps(), everything)
	case core.SourceFile:
		hits = v.Files.Search(m, v.files(), everything)
	case core.SourceClip:
		hits = v.clips().Search(m, v.clipboard(), everything)
	case core.SourceSnippet, core.SourceLink:
		hits = v.Library.Search(m, v.library(), everything)
	case core.SourceEmoji:
		hits = v.Emoji.Search(m, v.emoji(), everything)
	case core.SourceSystem:
		hits = system.Search(m, v.system())
	}
	for _, hit := range hits {
		if hit.Result.ID == id {
			decorate(v, &hit.Result)
			return hit, true
		}
	}
	return core.Scored{}, false
}

func TestReorderTiesRefusesARankingChange(t *testing.T) {
	scored := func(id core.ID, score uint32) core.Scored {
		return core.Scored{Score: score, Result: core.Result{ID: id}}
	}
	ids := func(hits []core.Scored) []core.ID {
		var out []core.ID
		for _, hit := range hits {
			out = append(out, hit.Result.ID)
		}
		return out
	}
	got := []core.Scored{scored("a", 9), scored("b", 5), scored("c", 5)}
	// Go ranks d and e but cut them: d ties with b and c, e does not.
	cut := map[core.ID]core.Scored{"d": scored("d", 5), "e": scored("e", 4)}
	hitOf := func(id core.ID) (core.Scored, bool) { hit, ok := cut[id]; return hit, ok }
	for _, want := range [][]core.ID{{"a", "c", "b"}, {"a", "b", "d"}, {"a", "d", "c"}} {
		tie, err := reorderTies(want, got, hitOf)
		if err != nil || !slices.Equal(ids(tie), want) {
			t.Errorf("%v as ties of %v: %v %v", want, ids(got), ids(tie), err)
		}
	}
	for _, want := range [][]core.ID{
		{"b", "a", "c"}, // b scores 5 in Go, below a's 9
		{"a", "b"},      // one is missing
		{"a", "b", "e"}, // e scores below c
		{"a", "b", "f"}, // Go does not find f
	} {
		if _, err := reorderTies(want, got, hitOf); err == nil {
			t.Errorf("%v passed as ties of %v", want, ids(got))
		}
	}
	// Answers and browse rows have no score to tie on.
	rows := []core.Scored{scored("x", unscored), scored("y", unscored)}
	if _, err := reorderTies([]core.ID{"y", "x"}, rows, hitOf); err == nil {
		t.Error("two unscored rows passed as ties")
	}
}

// Every category and query of testdata/golden/search.jsonl in the world
// that wrote it.
func TestSearchMatchesRust(t *testing.T) {
	// The corpus ran on macOS: its file manager is Finder, its app and file
	// icons are images, and its paths use slashes. Other OSes have their own.
	app := apps.NewIndex([]apps.App{{Name: "A", Path: "/A.app"}}).Browse(apps.Context{})[0]
	if app.Actions[1].Label != "Show in Finder" || app.Icon.File == "" || filepath.Separator != '/' {
		t.Skip("the corpus has the macOS labels and paths")
	}
	type row struct {
		Category core.Category     `json:"category"`
		Query    string            `json:"query"`
		Results  []json.RawMessage `json:"results"`
	}
	s := goldenWorld(t, golden.LoadMeta(t).Now(t))
	rows := golden.Rows[row](t, "search")
	if len(rows) != 7*40 {
		t.Fatalf("%d rows, want 280", len(rows))
	}
	for _, r := range rows {
		name := string(r.Category) + " " + golden.Quote(r.Query)
		var want []core.ID
		for _, raw := range r.Results {
			var result struct {
				ID core.ID `json:"id"`
			}
			if err := json.Unmarshal(raw, &result); err != nil {
				t.Fatal(err)
			}
			want = append(want, result.ID)
		}
		got := search(s, r.Query, r.Category)
		hitOf := func(id core.ID) (core.Scored, bool) { return sourceHit(s, r.Query, id) }
		if ordered, err := reorderTies(want, got, hitOf); err == nil {
			got = ordered
		} else {
			t.Logf("%s: not a reordering of ties: %v", name, err)
		}
		results := make([]any, len(got))
		for i, hit := range got {
			results[i] = golden.Result(hit.Result)
		}
		wantJSON, err := json.Marshal(r.Results)
		if err != nil {
			t.Fatal(err)
		}
		golden.Check(t, "search", name, wantJSON, results)
	}
}
