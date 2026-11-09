// Package files indexes file and folder names under the configured folders.
// The index stores paths only; it never reads file contents. The scan reads
// folders through a Disk, so this package stays free of the OS.
package files

import (
	"io/fs"
	"path/filepath"
	"slices"
	"strings"
	"unicode/utf8"

	"golang.org/x/text/unicode/norm"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

const (
	// Limit is the most entries an index holds; larger trees are cut off.
	Limit = 50_000
	// FoundLimit is the most paths outside the index that Open and Reveal
	// accept: those that Spotlight returned most recently.
	FoundLimit = 500
)

// Disk is what a scan reads.
type Disk interface {
	// ReadDir lists a folder. It follows a link in dir itself, but an
	// entry that is a link reports fs.ModeSymlink in its Type.
	ReadDir(dir string) ([]fs.DirEntry, error)
	// RealDir resolves the links in dir; ok is false unless dir is a
	// folder (after links). When links cannot be resolved, real is dir.
	RealDir(dir string) (real string, ok bool)
}

// Options says what a scan lists.
type Options struct {
	// Roots are the folders to scan.
	Roots []string
	// Excluded are folder names to skip, such as node_modules.
	Excluded []string
	// Packages are extensions (without the dot) of folders that are listed
	// without their contents, such as "app".
	Packages []string
	// Hidden is the OS check for hidden items, or nil when it has none.
	// Names that start with a dot are always hidden.
	Hidden func(path string) bool
}

// Personal is what ranking knows about the user. The usage bonus and pins
// live outside this package; a nil Personal means neither.
type Personal interface {
	// Boost is the usage bonus of a result.
	Boost(id core.ID) uint32
	Pinned(id core.ID) bool
}

// Context is how one search ranks and shows its results.
type Context struct {
	Personal Personal
	// Home is the user's home folder, shown as "~"; empty shows full paths.
	Home string
}

func (c Context) boost(id core.ID) uint32 {
	if c.Personal == nil {
		return 0
	}
	return c.Personal.Boost(id)
}

func (c Context) pinned(id core.ID) bool { return c.Personal != nil && c.Personal.Pinned(id) }

type entry struct {
	// path is in NFC, for matching: macOS can return decomposed accents.
	path string
	// original is the path as the OS spelled it, kept only when it differs
	// from path.
	original string
	// nameStart is the byte offset of the file name in path.
	nameStart int
	isDir     bool
}

func (e *entry) name() string { return e.path[e.nameStart:] }

func (e *entry) osPath() string {
	if e.original != "" {
		return e.original
	}
	return e.path
}

// Index is the files and folders a scan found.
type Index struct {
	entries []entry
	// byPath finds an entry by its OS path, for each Open and each used
	// file in an empty Files tab.
	byPath map[string]int
	// Truncated: the scan stopped at Limit.
	Truncated bool
}

// Scan walks the roots without following links, skipping hidden entries and
// excluded folder names. Unreadable folders are skipped. A root whose
// contents another root's scan lists is not scanned twice.
func Scan(disk Disk, o Options) *Index {
	s := scanner{disk: disk, o: o, x: &Index{}}
	for _, root := range distinctRoots(disk, o) {
		if !s.walk(root) {
			break
		}
	}
	s.x.byPath = make(map[string]int, len(s.x.entries))
	for i := range s.x.entries {
		s.x.byPath[s.x.entries[i].osPath()] = i
	}
	return s.x
}

type scanner struct {
	disk Disk
	o    Options
	x    *Index
}

// walk lists dir depth first, each folder before its contents. It returns
// false once the index is full.
func (s *scanner) walk(dir string) bool {
	children, err := s.disk.ReadDir(dir)
	if err != nil {
		return true
	}
	for _, child := range children {
		name := child.Name()
		path := join(dir, name)
		isDir := child.IsDir()
		if strings.HasPrefix(name, ".") || isDir && slices.Contains(s.o.Excluded, name) || s.o.Hidden != nil && s.o.Hidden(path) {
			continue
		}
		if len(s.x.entries) == Limit {
			s.x.Truncated = true
			return false
		}
		if child.Type()&fs.ModeSymlink != 0 {
			continue
		}
		// Paths that are not UTF-8 could not be shown or stored.
		if utf8.ValidString(path) {
			nfc := norm.NFC.String(path)
			e := entry{path: nfc, nameStart: strings.LastIndexByte(nfc, filepath.Separator) + 1, isDir: isDir}
			if nfc != path {
				e.original = path
			}
			s.x.entries = append(s.x.entries, e)
		}
		if isDir && !isPackage(name, s.o.Packages) && !s.walk(path) {
			return false
		}
	}
	return true
}

// Len is the number of entries.
func (x *Index) Len() int { return len(x.entries) }

// Contains reports whether the index lists path, as the OS spells it.
func (x *Index) Contains(path string) bool {
	_, ok := x.byPath[path]
	return ok
}

// Get is the result of the entry at path, as the OS spells it.
func (x *Index) Get(path string, ctx Context) (core.Result, bool) {
	i, ok := x.byPath[path]
	if !ok {
		return core.Result{}, false
	}
	return result(x.entries[i].osPath(), x.entries[i].isDir, ctx), true
}

// Search matches names; with two or more words or a separator, it also
// matches whole paths, so `project readme` finds project/README.md. The best
// limit come first.
func (x *Index) Search(m *match.Matcher, ctx Context, limit int) []core.Scored {
	query := m.Query()
	var paths *match.Matcher
	if strings.ContainsAny(query, " /\\") {
		paths = pathMatcher(query, filepath.Separator)
	}
	var hits []match.Hit[int]
	for i := range x.entries {
		e := &x.entries[i]
		score, ok := m.Name(e.name())
		if !ok && paths != nil {
			score, ok = paths.Words(e.path)
		}
		if ok {
			hits = append(hits, match.Hit[int]{Score: score, Item: i})
		}
	}
	// Rank by match first, then apply usage to the best candidates only.
	hits = match.Top(hits, limit*4)
	for i := range hits {
		hits[i].Score += ctx.boost(id(x.entries[hits[i].Item].osPath()))
	}
	top := match.Top(hits, limit)
	out := make([]core.Scored, len(top))
	for i, hit := range top {
		e := &x.entries[hit.Item]
		out[i] = core.Scored{Score: hit.Score, Result: result(e.osPath(), e.isDir, ctx)}
	}
	return out
}

// pathMatcher matches whole paths. Users type `/` between folders on every
// OS, so it becomes the path separator (`\` on Windows).
func pathMatcher(query string, separator rune) *match.Matcher {
	return match.New(strings.ReplaceAll(query, "/", string(separator)))
}

// Lists reports whether a scan of roots lists path: nothing on the way is
// hidden (a name that starts with a dot, or hidden says so), and no folder
// on the way is excluded or a package. The last part may be a file with an
// excluded name (isDir is false), or a package itself. The caller reads the
// disk for isDir and hidden.
func Lists(path string, isDir bool, roots, excluded, packages []string, hidden func(path string) bool) bool {
	for _, root := range roots {
		rest, ok := under(path, root)
		if !ok {
			continue
		}
		walked := root
		listed := true
		for i, name := range rest {
			walked = join(walked, name)
			parent := i+1 < len(rest)
			folder := parent || isDir
			if strings.HasPrefix(name, ".") || folder && slices.Contains(excluded, name) ||
				parent && isPackage(name, packages) || hidden != nil && hidden(walked) {
				listed = false
				break
			}
		}
		if listed {
			return true
		}
	}
	return false
}

// distinctRoots drops roots whose contents a scan of another root already
// lists, so nothing is scanned twice and nothing is missed. Roots are
// compared by real path: the walk follows a root that is a link but no link
// inside it, so ~/Dropbox, a link to a folder under ~/Library, is covered by
// ~ while a link to another disk is not.
func distinctRoots(disk Disk, o Options) []string {
	type root struct{ real, folder string }
	var roots []root
	for _, folder := range o.Roots {
		if real, ok := disk.RealDir(folder); ok {
			roots = append(roots, root{real, folder})
		}
	}
	slices.SortFunc(roots, func(a, b root) int {
		if c := comparePaths(a.real, b.real); c != 0 {
			return c
		}
		return comparePaths(a.folder, b.folder)
	})
	roots = slices.CompactFunc(roots, func(a, b root) bool { return a.real == b.real })
	var kept, distinct []string
	for _, r := range roots {
		covered := !isPackage(base(r.real), o.Packages) && Lists(r.real, true, kept, o.Excluded, o.Packages, o.Hidden)
		if !covered {
			kept = append(kept, r.real)
			distinct = append(distinct, r.folder)
		}
	}
	return distinct
}

// isPackage reports whether a name has one of the package extensions. A
// name like ".app" has no extension.
func isPackage(name string, packages []string) bool {
	dot := strings.LastIndexByte(name, '.')
	if dot <= 0 {
		return false
	}
	return slices.ContainsFunc(packages, func(p string) bool { return strings.EqualFold(name[dot+1:], p) })
}

func isSeparator(c byte) bool { return c == '/' || c == filepath.Separator }

// join appends a name to a folder path with one separator.
func join(dir, name string) string {
	if dir == "" || isSeparator(dir[len(dir)-1]) {
		return dir + name
	}
	return dir + string(filepath.Separator) + name
}

// parts splits a path into its volume, its root, and its names, as Rust's
// Path::components does, so "/a/b/" and "/a//b" are the same path.
func parts(path string) []string {
	volume := filepath.VolumeName(path)
	rest := path[len(volume):]
	var out []string
	if volume != "" {
		out = append(out, volume)
	}
	if rest != "" && isSeparator(rest[0]) {
		out = append(out, string(filepath.Separator))
	}
	for i, name := range strings.FieldsFunc(rest, func(r rune) bool { return r < utf8.RuneSelf && isSeparator(byte(r)) }) {
		if name != "." || i == 0 && len(out) == 0 {
			out = append(out, name)
		}
	}
	return out
}

func comparePaths(a, b string) int { return slices.Compare(parts(a), parts(b)) }

// under returns the names that follow root in path.
func under(path, root string) ([]string, bool) {
	p, r := parts(path), parts(root)
	if len(p) < len(r) || !slices.Equal(p[:len(r)], r) {
		return nil, false
	}
	return p[len(r):], true
}

// base is the last name in a path, or "" when it has none.
func base(path string) string {
	p := parts(path)
	if len(p) == 0 {
		return ""
	}
	last := p[len(p)-1]
	if last == ".." || last == string(filepath.Separator) || last == filepath.VolumeName(path) {
		return ""
	}
	return last
}

// parent is the folder that holds path, or "" for a root or a bare name.
func parent(path string) string {
	dir := filepath.Dir(path)
	if dir == path || dir == "." {
		return ""
	}
	return dir
}

// DisplayPath shows a path in NFC, with the home folder as "~".
func DisplayPath(path, home string) string {
	if home != "" {
		if path == home {
			return "~"
		}
		prefix := home
		if !strings.HasSuffix(prefix, string(filepath.Separator)) {
			prefix += string(filepath.Separator)
		}
		if rest, ok := strings.CutPrefix(path, prefix); ok {
			if rest == "" {
				return "~"
			}
			path = "~" + string(filepath.Separator) + rest
		}
	}
	return norm.NFC.String(path)
}

func id(path string) core.ID { return core.SourceFile.ID(path) }

func result(path string, isDir bool, ctx Context) core.Result {
	r := FoundResult(path, isDir, ctx.Home)
	r.Pinned = ctx.pinned(r.ID)
	if r.Pinned {
		r.Actions = append(r.Actions, core.Do("Unpin", core.Unpin{ID: r.ID}))
	} else {
		r.Actions = append(r.Actions, core.Do("Pin", core.Pin{ID: r.ID}))
	}
	return r
}

// FoundResult is a file or folder that Spotlight found outside the index.
// It has no Pin: a pin resolves only through the index.
func FoundResult(path string, isDir bool, home string) core.Result {
	title := norm.NFC.String(base(path))
	if title == "" {
		title = path
	}
	kind, symbol := core.KindFile, core.SymbolFile
	if isDir {
		kind, symbol = core.KindFolder, core.SymbolFolder
	}
	icon := core.SymbolIcon(symbol)
	if nativeIcons {
		icon = core.FileIcon(path)
	}
	return core.Result{
		ID:       id(path),
		Kind:     kind,
		Title:    title,
		Subtitle: DisplayPath(parent(path), home),
		Icon:     icon,
		Actions: []core.ResultAction{
			core.Do("Open", core.Open{Path: path}),
			core.Do("Show in "+fileManager, core.Reveal{Path: path}),
			core.Do("Copy Path", core.Copy{Text: path}),
		},
	}
}

// FoundPaths are the paths that Spotlight searches returned, newest last.
// TinyDash offered them, so Open and Reveal accept them like indexed ones.
type FoundPaths struct {
	paths []string
}

// Remember adds a path as the newest, dropping the oldest past FoundLimit.
func (f *FoundPaths) Remember(path string) {
	f.paths = slices.DeleteFunc(f.paths, func(known string) bool { return known == path })
	if len(f.paths) >= FoundLimit {
		f.paths = slices.Delete(f.paths, 0, 1)
	}
	f.paths = append(f.paths, path)
}

// Contains reports whether a search returned path.
func (f *FoundPaths) Contains(path string) bool { return slices.Contains(f.paths, path) }
