package app

import (
	"context"
	"slices"
	"sync"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
)

// fakeIndex stands in for the OS file index: each search waits for its
// answer on the channel of its name, so no test runs Spotlight.
type fakeIndex struct {
	mu      sync.Mutex
	answers map[string]chan []string
	// asked gets the name of each search as it starts.
	asked chan string
}

func newFakeIndex(names ...string) *fakeIndex {
	f := &fakeIndex{answers: map[string]chan []string{}, asked: make(chan string, 8)}
	for _, name := range names {
		f.answers[name] = make(chan []string, 1)
	}
	return f
}

func (f *fakeIndex) find(ctx context.Context, name, folder string, limit int) []string {
	f.mu.Lock()
	answer := f.answers[name]
	f.mu.Unlock()
	f.asked <- name
	select {
	case paths := <-answer:
		return paths
	case <-ctx.Done():
		return nil
	}
}

func spotlightSnapshot() *search.Snapshot {
	s := settings.Default()
	s.SpotlightFiles = true
	return &search.Snapshot{Settings: s, Files: files.Scan(nil, files.Options{}), Home: golden.Abs("/home/me")}
}

func newTestMoreFiles(index *fakeIndex, folders map[string]bool) (*moreFiles, *[]string, chan string) {
	var remembered []string
	answered := make(chan string, 4)
	m := &moreFiles{
		find: index.find,
		isDir: func(path string) (bool, bool) {
			dir, ok := folders[golden.Slash(path)]
			return dir, ok
		},
		remember: func(path string) { remembered = append(remembered, golden.Slash(path)) },
	}
	m.done = func() {
		m.mu.Lock()
		defer m.mu.Unlock()
		answered <- m.asked
	}
	return m, &remembered, answered
}

func slashIDs(results []core.Result) []string {
	var out []string
	for _, r := range results {
		out = append(out, golden.Slash(string(r.ID)))
	}
	return out
}

func TestOSFileResultsFollowTheIndexRulesAndAreRemembered(t *testing.T) {
	index := newFakeIndex("notes")
	m, remembered, answered := newTestMoreFiles(index, map[string]bool{
		"/home/me/notes.txt":                false,
		"/home/me/.secret/notes.txt":        false,
		"/home/me/node_modules/x/notes.txt": false,
		"/home/me/Notes":                    true,
	})
	m.packages = []string{"app"}
	s := spotlightSnapshot()
	if got := m.results(context.Background(), s, "notes", core.CategoryFiles); len(got) != 0 {
		t.Fatalf("rows before the OS answered: %v", slashIDs(got))
	}
	index.answers["notes"] <- []string{
		golden.Abs("/home/me/notes.txt"),
		golden.Abs("/home/me/.secret/notes.txt"),        // hidden by its dot
		golden.Abs("/home/me/node_modules/x/notes.txt"), // in a folder to skip
		golden.Abs("/home/me/gone/notes.txt"),           // removed since
		golden.Abs("/home/me/Notes"),
	}
	if got := <-answered; got != "notes" {
		t.Fatalf("answered %q", got)
	}
	got := slashIDs(m.results(context.Background(), s, "notes", core.CategoryFiles))
	want := []string{"file:/home/me/Notes", "file:/home/me/notes.txt"}
	if !slices.Equal(got, want) {
		t.Errorf("rows %q, want %q", got, want)
	}
	if want := []string{"/home/me/Notes", "/home/me/notes.txt"}; !slices.Equal(*remembered, want) {
		t.Errorf("remembered %q, want %q", *remembered, want)
	}
	if n := len(index.asked); n != 1 {
		t.Errorf("asked the OS %d times for one query", n)
	}
}

func TestOnlyTheLatestOSFileSearchAnswers(t *testing.T) {
	index := newFakeIndex("no", "not")
	m, _, answered := newTestMoreFiles(index, map[string]bool{"/home/me/no.txt": false, "/home/me/not.txt": false})
	s := spotlightSnapshot()
	m.results(context.Background(), s, "no", core.CategoryFiles)
	m.results(context.Background(), s, "not", core.CategoryFiles)
	index.answers["no"] <- []string{golden.Abs("/home/me/no.txt")}
	index.answers["not"] <- []string{golden.Abs("/home/me/not.txt")}
	if got := <-answered; got != "not" {
		t.Fatalf("answered %q, want the latest search", got)
	}
	got := slashIDs(m.results(context.Background(), s, "not", core.CategoryFiles))
	if want := []string{"file:/home/me/not.txt"}; !slices.Equal(got, want) {
		t.Errorf("rows %q, want %q", got, want)
	}
}

func TestOSFileSearchStopsOutsideTheFilesTabAndAsksAgainOnReturn(t *testing.T) {
	index := newFakeIndex("notes")
	m, _, answered := newTestMoreFiles(index, map[string]bool{"/home/me/notes.txt": false})
	s := spotlightSnapshot()
	m.results(context.Background(), s, "notes", core.CategoryFiles)
	index.answers["notes"] <- []string{golden.Abs("/home/me/notes.txt")}
	<-answered
	if got := m.results(context.Background(), s, "notes", core.CategoryAll); len(got) != 0 {
		t.Errorf("All tab got OS rows %v", slashIDs(got))
	}
	if got := m.results(context.Background(), s, "notes", core.CategoryFiles); len(got) != 0 {
		t.Errorf("rows of a forgotten search: %v", slashIDs(got))
	}
	for range 2 {
		if name := <-index.asked; name != "notes" {
			t.Errorf("asked for %q", name)
		}
	}
}
