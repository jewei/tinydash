package search

import (
	"io/fs"
	"slices"
	"testing"
	"testing/fstest"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/golden"
)

// oneFolder is a disk with one folder, holding the files of fsys.
type oneFolder struct {
	dir  string
	fsys fstest.MapFS
}

func (d oneFolder) ReadDir(dir string) ([]fs.DirEntry, error) {
	if dir != d.dir {
		return nil, fs.ErrNotExist
	}
	return fs.ReadDir(d.fsys, ".")
}

func (d oneFolder) RealDir(dir string) (string, bool) { return dir, dir == d.dir }

func TestMoreFilesAsksOnlyInTheFilesTabWithTwoCharactersAndAHome(t *testing.T) {
	on := empty()
	on.Settings.SpotlightFiles = true
	on.Home = golden.Abs("/home/me")
	off := empty()
	off.Home = on.Home
	noHome := empty()
	noHome.Settings.SpotlightFiles = true
	for _, c := range []struct {
		name     string
		s        *Snapshot
		query    string
		category core.Category
		want     string
		ok       bool
	}{
		{"on", on, "  readme ", core.CategoryFiles, "readme", true},
		{"two characters, counted as runes", on, "日本", core.CategoryFiles, "日本", true},
		{"one character", on, " r ", core.CategoryFiles, "r", false},
		{"another tab", on, "readme", core.CategoryAll, "readme", false},
		{"setting off", off, "readme", core.CategoryFiles, "readme", false},
		{"no home folder", noHome, "readme", core.CategoryFiles, "readme", false},
	} {
		name, ok := MoreFilesQuery(c.s, c.query, c.category)
		if name != c.want || ok != c.ok {
			t.Errorf("%s: got %q %v, want %q %v", c.name, name, ok, c.want, c.ok)
		}
	}
}

func TestMoreFilesSkipsIndexedAndHiddenPathsAndRanksByName(t *testing.T) {
	s := empty()
	indexed := golden.Abs("/home/me/indexed")
	s.Files = files.Scan(oneFolder{indexed, fstest.MapFS{"notes.txt": {}}}, files.Options{Roots: []string{indexed}})
	hidden := golden.Abs("/home/me/hidden.txt")
	s.Hidden = NewHidden(core.SourceFile.ID(hidden))
	found := []FoundFile{
		{Path: hidden},
		{Path: golden.Abs("/home/me/indexed/notes.txt")},
		{Path: golden.Abs("/home/me/old/a-notes.txt")},
		{Path: golden.Abs("/home/me/notes"), IsDir: true},
		{Path: golden.Abs("/home/me/zz/b-notes.txt")},
	}
	var got []string
	for _, f := range MoreFiles(s, "notes", found) {
		got = append(got, golden.Slash(f.Path))
	}
	// The exact name ranks first; equal scores keep the OS's order.
	want := []string{"/home/me/notes", "/home/me/old/a-notes.txt", "/home/me/zz/b-notes.txt"}
	if !slices.Equal(got, want) {
		t.Errorf("got %q, want %q", got, want)
	}
}
