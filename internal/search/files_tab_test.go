package search

import (
	"fmt"
	"io/fs"
	"slices"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/golden"
)

// slashDisk is a files.Disk of slash paths, so a scan rooted at
// golden.Abs walks the same tree on Windows.
type slashDisk struct{ fsys fstest.MapFS }

func (d slashDisk) ReadDir(dir string) ([]fs.DirEntry, error) {
	return fs.ReadDir(d.fsys, strings.TrimPrefix(golden.Slash(dir), "/"))
}

func (d slashDisk) RealDir(dir string) (string, bool) {
	info, err := fs.Stat(d.fsys, strings.TrimPrefix(golden.Slash(dir), "/"))
	return dir, err == nil && info.IsDir()
}

// fileWorld indexes rels, each a slash path under /root, such as
// "docs/notes.txt".
func fileWorld(t *testing.T, rels []string) *Snapshot {
	t.Helper()
	fsys := fstest.MapFS{"root": {Mode: fs.ModeDir}}
	for _, rel := range rels {
		fsys["root/"+rel] = &fstest.MapFile{}
	}
	s := empty()
	s.Files = files.Scan(slashDisk{fsys}, files.Options{Roots: []string{golden.Abs("/root")}})
	return s
}

func fileID(t *testing.T, s *Snapshot, rel string) core.ID {
	t.Helper()
	path := golden.Abs("/root/" + rel)
	r, ok := s.Files.Get(path, files.Context{})
	if !ok || r.Title != rel[strings.LastIndex(rel, "/")+1:] {
		t.Fatalf("%s: indexed as %+v, %v", rel, r, ok)
	}
	return r.ID
}

// The empty Files tab is the files the user pinned and the files they
// opened. Listing the index would put tens of thousands of names in the
// tab, ahead of the ones they came back for.
func TestTheFilesTabListsPinsThenFilesOpenedBefore(t *testing.T) {
	s := fileWorld(t, []string{
		"Photos/beach.png",
		"docs/budget.xlsx",
		"docs/notes.txt",
		"Projects/tinydash/README.md",
	})
	beach := fileID(t, s, "Photos/beach.png")
	budget := fileID(t, s, "docs/budget.xlsx")
	notes := fileID(t, s, "docs/notes.txt")
	readme := fileID(t, s, "Projects/tinydash/README.md")
	now := s.Now.Unix()
	s.Usage = NewUsage(map[core.ID]core.Use{
		notes:              {Count: 9, LastUsed: now},
		beach:              {Count: 2, LastUsed: now},
		budget:             {Count: 1, LastUsed: now},
		core.ID("emoji:🚀"): {Count: 20, LastUsed: now},
		core.SourceFile.ID(golden.Abs("/root/gone.txt")): {Count: 20, LastUsed: now},
	})
	// readme was never opened. Its pin is still the user's choice.
	s.Pins = NewPins([]core.ID{readme, "emoji:🚀", budget})

	if got := ids(Search(s, "", core.CategoryFiles)); !slices.Equal(got, []core.ID{readme, budget, notes, beach}) {
		t.Fatalf("empty Files tab %q", got)
	}
	pinned := Search(s, "", core.CategoryFiles)[0]
	if !pinned.Pinned || !hasAction(pinned, core.Unpin{ID: readme}) || pinned.Actions[0].Action != (core.Open{Path: golden.Abs("/root/Projects/tinydash/README.md")}) {
		t.Errorf("pinned row %+v", pinned)
	}
	opened := Search(s, "", core.CategoryFiles)[2]
	if opened.ID != notes || opened.Pinned || !hasAction(opened, core.Pin{ID: notes}) {
		t.Errorf("opened row %+v", opened)
	}

	s.Hidden = NewHidden(beach)
	if slices.Contains(ids(Search(s, "", core.CategoryFiles)), beach) {
		t.Error("a hidden file stays in the empty Files tab")
	}

	// Out of All, the tab is unchanged and All stops suggesting files.
	s.Hidden = NewHidden()
	withTabs(s, func(c core.Category) bool { return c != core.CategoryFiles })
	if got := ids(Search(s, "", core.CategoryFiles)); !slices.Equal(got, []core.ID{readme, budget, notes, beach}) {
		t.Errorf("Files tab with files out of All: %q", got)
	}
	// A pin stays on the empty All view. A file the user only opened does not:
	// that is a suggestion, and suggestions leave All with their source.
	all := ids(Search(s, "", core.CategoryAll))
	if !slices.Contains(all, readme) || !slices.Contains(all, budget) || slices.Contains(all, notes) || slices.Contains(all, beach) {
		t.Errorf("All with files out of All: %q", all)
	}
}

func TestTheFilesTabStopsAfterTheMostRecentlyOpened(t *testing.T) {
	rels := make([]string, CategoryLimit+1)
	for i := range rels {
		rels[i] = fmt.Sprintf("f%03d.txt", i)
	}
	s := fileWorld(t, rels)
	now := s.Now.Unix()
	usage := make(map[core.ID]core.Use, len(rels))
	for i, rel := range rels {
		// f100 was opened today, f000 a hundred days ago. Counts match, so
		// recency alone decides, and that order is the reverse of the names.
		usage[fileID(t, s, rel)] = core.Use{Count: 1, LastUsed: now - int64(CategoryLimit-i)*day}
	}
	s.Usage = NewUsage(usage)

	got := ids(Search(s, "", core.CategoryFiles))
	oldest := fileID(t, s, "f000.txt")
	if len(got) != CategoryLimit || got[0] != fileID(t, s, "f100.txt") || slices.Contains(got, oldest) {
		t.Fatalf("%d rows, first %s, has oldest %v", len(got), got[0], slices.Contains(got, oldest))
	}
}

func TestTheFilesTabFindsNamesPathsAndAliases(t *testing.T) {
	s := fileWorld(t, []string{
		"docs/notes.txt",
		"docs/notes.md",
		"Projects/tinydash/README.md",
		"docs/budget.xlsx",
		"docs/report.pdf",
	})
	notes := fileID(t, s, "docs/notes.txt")
	other := fileID(t, s, "docs/notes.md")
	readme := fileID(t, s, "Projects/tinydash/README.md")
	budget := fileID(t, s, "docs/budget.xlsx")
	report := fileID(t, s, "docs/report.pdf")
	s.Usage = NewUsage(map[core.ID]core.Use{
		notes: {Count: 20, LastUsed: s.Now.Unix()},
	})
	s.Aliases = NewAliases(map[core.ID]string{report: "budget"})

	found := ids(Search(s, "notes", core.CategoryFiles))
	if len(found) < 2 || found[0] != notes || !slices.Contains(found, other) {
		t.Fatalf("notes: %q", found)
	}
	// Words match anywhere in the path. A slash keeps the names next to
	// each other, so projects/readme does not find Projects/tinydash/README.md.
	if got := ids(Search(s, "projects readme", core.CategoryFiles)); !slices.Equal(got, []core.ID{readme}) {
		t.Errorf("projects readme: %q", got)
	}
	if got := ids(Search(s, "tinydash/readme", core.CategoryFiles)); !slices.Equal(got, []core.ID{readme}) {
		t.Errorf("tinydash/readme: %q", got)
	}
	if got := Search(s, "projects/readme", core.CategoryFiles); len(got) != 0 {
		t.Errorf("projects/readme: %q", titles(got))
	}
	// The whole alias goes first, ahead of the file whose name matches.
	if got := ids(Search(s, "budget", core.CategoryFiles)); len(got) < 2 || got[0] != report || !slices.Contains(got, budget) {
		t.Fatalf("budget: %q", got)
	}
	if got := ids(Search(s, "bud", core.CategoryFiles)); !slices.Contains(got, report) || !slices.Contains(got, budget) {
		t.Errorf("bud: %q", got)
	}
	named := Search(s, "report", core.CategoryFiles)
	if len(named) != 1 || !hasAction(named[0], core.SetAlias{ID: report, Alias: "budget"}) {
		t.Errorf("report row %+v", named)
	}
	if slices.Contains(ids(Search(s, "budget", core.CategoryApps)), report) {
		t.Error("a file alias matches in the Apps tab")
	}
	if got := Search(s, "zzzz-no-file", core.CategoryFiles); len(got) != 0 {
		t.Errorf("no match: %q", titles(got))
	}

	s.Hidden = NewHidden(budget)
	if got := ids(Search(s, "budget", core.CategoryFiles)); slices.Contains(got, budget) || !slices.Equal(got, []core.ID{report}) {
		t.Errorf("with budget hidden: %q", got)
	}

	s.Hidden = NewHidden()
	withTabs(s, func(c core.Category) bool { return c != core.CategoryFiles })
	if got := ids(Search(s, "budget", core.CategoryFiles)); !slices.Contains(got, report) || !slices.Contains(got, budget) {
		t.Errorf("Files tab with files out of All: %q", got)
	}
	for _, id := range ids(Search(s, "budget", core.CategoryAll)) {
		if strings.HasPrefix(string(id), "file:") {
			t.Errorf("All finds %s while files are out of All", id)
		}
	}
}

func hasAction(r core.Result, want core.Action) bool {
	for _, a := range r.Actions {
		if a.Action == want {
			return true
		}
	}
	return false
}
