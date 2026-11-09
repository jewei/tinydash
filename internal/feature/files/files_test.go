package files

import (
	"fmt"
	"io/fs"
	"slices"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// disk serves a MapFS at "/", with links: a link path to its target.
type disk struct {
	fsys  fstest.MapFS
	links map[string]string
}

func (d disk) resolve(path string) string {
	for link, target := range d.links {
		if path == link || strings.HasPrefix(path, link+"/") {
			return target + path[len(link):]
		}
	}
	return path
}

func (d disk) ReadDir(dir string) ([]fs.DirEntry, error) {
	return fs.ReadDir(d.fsys, strings.TrimPrefix(d.resolve(dir), "/"))
}

func (d disk) RealDir(dir string) (string, bool) {
	real := d.resolve(dir)
	info, err := fs.Stat(d.fsys, strings.TrimPrefix(real, "/"))
	return real, err == nil && info.IsDir()
}

// tree makes a disk with these files and folders (a trailing "/") under
// /root, and the links as fs.ModeSymlink entries.
func tree(paths []string, links map[string]string) disk {
	fsys := fstest.MapFS{}
	for _, p := range paths {
		if dir, ok := strings.CutSuffix(p, "/"); ok {
			fsys["root/"+dir] = &fstest.MapFile{Mode: fs.ModeDir}
		} else {
			fsys["root/"+p] = &fstest.MapFile{}
		}
	}
	for link, target := range links {
		fsys[strings.TrimPrefix(link, "/")] = &fstest.MapFile{Mode: fs.ModeSymlink, Data: []byte(target)}
	}
	return disk{fsys, links}
}

func hiddenSet(paths ...string) func(string) bool {
	return func(path string) bool { return slices.Contains(paths, path) }
}

func TestScansNamesSkipsHiddenAndExcludedAndMatchesPaths(t *testing.T) {
	d := tree([]string{
		"docs/project/README.md", "node_modules/pkg/index.js", ".hidden/secret.txt",
		"notes/todo.txt", "notes/hidden.txt", "Tool.APP/Contents/Info.plist",
	}, map[string]string{"/root/notes/link": "/root/docs"})
	x := Scan(d, Options{
		Roots:    []string{"/root", "/root/notes"},
		Excluded: []string{"node_modules"},
		Packages: []string{"app"},
		Hidden:   hiddenSet("/root/notes/hidden.txt"),
	})
	var names []string
	for i := range x.entries {
		names = append(names, x.entries[i].name())
	}
	slices.Sort(names)
	want := []string{"README.md", "Tool.APP", "docs", "notes", "project", "todo.txt"}
	if !slices.Equal(names, want) || x.Truncated {
		t.Fatalf("names %q, truncated %v", names, x.Truncated)
	}
	titles := func(query string) []string {
		var out []string
		for _, hit := range x.Search(match.New(query), Context{}, 10) {
			out = append(out, hit.Result.Title)
		}
		return out
	}
	if got := titles("readme"); !slices.Equal(got, []string{"README.md"}) {
		t.Errorf("readme: %q", got)
	}
	if got := titles("project readme"); len(got) == 0 || got[0] != "README.md" {
		t.Errorf("project readme: %q", got)
	}
	if got := titles("project/readme"); !slices.Equal(got, []string{"README.md"}) {
		t.Errorf("project/readme: %q", got)
	}
	for i := range x.entries {
		if !x.Contains(x.entries[i].osPath()) {
			t.Errorf("does not contain %s", x.entries[i].osPath())
		}
	}
	if !x.Contains("/root/notes/todo.txt") || x.Contains("/root/notes/missing.txt") {
		t.Error("contains is wrong")
	}
}

func TestStopsAtTheLimit(t *testing.T) {
	var paths []string
	for n := range Limit + 1 {
		paths = append(paths, fmt.Sprintf("f%d", n))
	}
	x := Scan(tree(paths, nil), Options{Roots: []string{"/root"}})
	if x.Len() != Limit || !x.Truncated {
		t.Errorf("len %d, truncated %v", x.Len(), x.Truncated)
	}
	x = Scan(tree(paths[:Limit], nil), Options{Roots: []string{"/root"}})
	if x.Len() != Limit || x.Truncated {
		t.Errorf("len %d, truncated %v", x.Len(), x.Truncated)
	}
}

func TestKeepsTheOSSpellingOfDecomposedNames(t *testing.T) {
	decomposed := "Cafe\u0301.txt"
	x := Scan(tree([]string{decomposed}, nil), Options{Roots: []string{"/root"}})
	hits := x.Search(match.New("caf\u00e9"), Context{}, 10)
	if len(hits) != 1 {
		t.Fatalf("%d hits", len(hits))
	}
	r := hits[0].Result
	if r.ID != core.ID("file:/root/"+decomposed) || r.Title != "Caf\u00e9.txt" || r.Actions[0].Action != (core.Open{Path: "/root/" + decomposed}) {
		t.Errorf("%+v", r)
	}
	if _, ok := x.Get("/root/"+decomposed, Context{}); !ok {
		t.Error("Get by the OS spelling failed")
	}
}

func TestPathQueriesTypedWithSlashesMatchBackslashPaths(t *testing.T) {
	path := `C:\Users\me\Documents\docs\readme.md`
	if _, ok := match.New("docs/readme").Words(path); ok {
		t.Error("a slash query matched a backslash path without the path matcher")
	}
	for _, query := range []string{"docs/readme", `docs\readme`, "docs/ readme"} {
		if _, ok := pathMatcher(query, '\\').Words(path); !ok {
			t.Errorf("%q", query)
		}
	}
	path = `C:\Users\café\文档\readme.md`
	for _, query := range []string{"café/文档", "文档/readme"} {
		if _, ok := pathMatcher(query, '\\').Words(path); !ok {
			t.Errorf("%q", query)
		}
	}
	if _, ok := pathMatcher("docs/readme", '/').Words("/Users/me/docs/readme.md"); !ok {
		t.Error("slash paths")
	}
}

func TestDisplayPathsShortenTheHomeFolder(t *testing.T) {
	for _, c := range []struct{ path, want string }{
		{"/Users/me", "~"},
		{"/Users/me/Notes", "~/Notes"},
		{"/Users/me2/Notes", "/Users/me2/Notes"},
		{"/Applications", "/Applications"},
		{"/Users/me/Cafe\u0301", "~/Caf\u00e9"},
	} {
		if got := DisplayPath(c.path, "/Users/me"); got != c.want {
			t.Errorf("%q: %q, want %q", c.path, got, c.want)
		}
	}
}

func TestListsOnlyWhatTheScanWalksInto(t *testing.T) {
	roots := []string{"/nowhere/Documents"}
	listed := func(rest string) bool {
		return Lists(roots[0]+"/"+rest, false, roots, []string{"target"}, []string{"app"},
			func(path string) bool { return strings.HasSuffix(path, "/Library") })
	}
	for rest, want := range map[string]bool{
		"notes/todo.txt": true, "Tool.app": true, "notes/target": true,
		".git/HEAD": false, "target/debug/build": false, "Library/Caches/x": false,
		"Tool.app/Contents/Info.plist": false,
	} {
		if listed(rest) != want {
			t.Errorf("%s: listed %v", rest, !want)
		}
	}
	if Lists("/elsewhere/a.txt", false, roots, nil, nil, nil) {
		t.Error("lists a path outside the roots")
	}
}

func TestNestedRootsAreScannedOnceUnlessTheOuterScanSkipsThem(t *testing.T) {
	d := tree([]string{"docs/", ".config/", "target/", "Tool.app/"}, nil)
	var folders []string
	for _, name := range []string{"docs", ".config", "target", "Tool.app", ""} {
		folders = append(folders, strings.TrimSuffix("/root/"+name, "/"))
	}
	kept := distinctRoots(d, Options{Roots: folders, Excluded: []string{"target"}, Packages: []string{"app"}})
	slices.Sort(kept)
	want := []string{"/root", "/root/.config", "/root/Tool.app", "/root/target"}
	if !slices.Equal(kept, want) {
		t.Errorf("kept %q, want %q", kept, want)
	}
}

func TestANestedRootUnderAHiddenFolderIsScanned(t *testing.T) {
	d := tree([]string{"AppData/Roaming/Notes/"}, nil)
	folders := []string{"/root", "/root/AppData/Roaming/Notes"}
	kept := distinctRoots(d, Options{Roots: folders, Hidden: hiddenSet("/root/AppData")})
	if !slices.Equal(kept, folders) {
		t.Errorf("kept %q", kept)
	}
	if kept := distinctRoots(d, Options{Roots: folders}); !slices.Equal(kept, folders[:1]) {
		t.Errorf("kept %q without the hidden check", kept)
	}
}

func TestANestedRootReachedThroughALinkIsComparedByRealPath(t *testing.T) {
	// A link to a folder the outer scan lists adds nothing; a link to a
	// folder outside it is scanned on its own.
	d := tree([]string{"Library/Dropbox/"}, map[string]string{
		"/root/Dropbox":  "/root/Library/Dropbox",
		"/root/External": "/elsewhere",
	})
	d.fsys["elsewhere/a.txt"] = &fstest.MapFile{}
	folders := []string{"/root", "/root/Dropbox", "/root/External"}
	kept := distinctRoots(d, Options{Roots: folders})
	slices.Sort(kept)
	if want := []string{"/root", "/root/External"}; !slices.Equal(kept, want) {
		t.Errorf("kept %q, want %q", kept, want)
	}
	x := Scan(d, Options{Roots: folders})
	if !x.Contains("/root/External/a.txt") || x.Contains("/root/Dropbox") || x.Contains("/root/External") {
		t.Error("the scan followed a link inside a root, or not a link that is a root")
	}
}

func TestFoundPathsKeepTheNewestAndNoRepeats(t *testing.T) {
	var found FoundPaths
	for n := range FoundLimit + 2 {
		found.Remember(fmt.Sprintf("/f%d", n))
	}
	if found.Contains("/f0") || found.Contains("/f1") || !found.Contains("/f2") || !found.Contains(fmt.Sprintf("/f%d", FoundLimit+1)) {
		t.Error("kept the wrong paths")
	}
	found.Remember("/f2")
	if found.paths[len(found.paths)-1] != "/f2" || len(found.paths) != FoundLimit {
		t.Errorf("newest %q, %d paths", found.paths[len(found.paths)-1], len(found.paths))
	}
}

// The shape the Rust app gave, from testdata/golden/search.jsonl.
func TestResultShape(t *testing.T) {
	x := Scan(tree([]string{"Documents/report.pdf"}, nil), Options{Roots: []string{"/root"}})
	r, ok := x.Get("/root/Documents/report.pdf", Context{})
	if !ok {
		t.Fatal("not found")
	}
	if r.ID != "file:/root/Documents/report.pdf" || r.Kind != core.KindFile || r.Title != "report.pdf" || r.Subtitle != "/root/Documents" {
		t.Errorf("%+v", r)
	}
	var labels []string
	for _, a := range r.Actions {
		labels = append(labels, a.Label)
	}
	if want := []string{"Open", "Show in " + fileManager, "Copy Path", "Pin"}; !slices.Equal(labels, want) {
		t.Errorf("labels %q", labels)
	}
	folder, _ := x.Get("/root/Documents", Context{Home: "/root"})
	if folder.Kind != core.KindFolder || folder.Subtitle != "~" {
		t.Errorf("%+v", folder)
	}
	if found := FoundResult("/root/a.txt", false, ""); len(found.Actions) != 3 || found.Pinned {
		t.Errorf("found result %+v", found)
	}
}
