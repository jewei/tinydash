package search

import (
	"cmp"
	"path/filepath"
	"slices"
	"strings"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

const (
	// moreFilesMinChars: shorter queries match too much to be useful.
	moreFilesMinChars = 2
	// MoreFilesLimit is the most paths one OS file search adds.
	MoreFilesLimit = 100
)

// FoundFile is a path that the OS's file index returned and that the
// index's rules list (files.Lists), as the app read it from the disk.
type FoundFile struct {
	Path  string
	IsDir bool
}

// MoreFilesQuery is the name to ask the OS's file index for, after the
// Files tab's own results: the trimmed query, when Spotlight search is on,
// the tab is Files, the query is long enough, and the home folder, where
// the OS search looks, is known.
func MoreFilesQuery(s *Snapshot, query string, category core.Category) (string, bool) {
	name := strings.TrimSpace(query)
	ok := s.Settings.SpotlightFiles && category == core.CategoryFiles &&
		utf8.RuneCountInString(name) >= moreFilesMinChars && filepath.IsAbs(s.Home)
	return name, ok
}

// MoreFiles are the found paths that the Files tab shows after its own
// results: not in the index, which lists them already, and not hidden by
// the user. The matcher ranks them by name, as the index would.
func MoreFiles(s *Snapshot, name string, found []FoundFile) []FoundFile {
	m := match.New(name)
	type hit struct {
		score uint32
		file  FoundFile
	}
	var hits []hit
	for _, f := range found {
		if s.Files.Contains(f.Path) || s.Hidden.Contains(core.SourceFile.ID(f.Path)) {
			continue
		}
		score, _ := m.Name(filepath.Base(f.Path))
		hits = append(hits, hit{score, f})
	}
	slices.SortStableFunc(hits, func(a, b hit) int { return cmp.Compare(b.score, a.score) })
	out := make([]FoundFile, len(hits))
	for i, h := range hits {
		out[i] = h.file
	}
	return out
}
