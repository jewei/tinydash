// Package archtest checks the import rules of docs/go-architecture.md. The
// rules are data in this file; archtest_test.go runs them over the real
// module with `go list` and the standard library only.
package archtest

import (
	"fmt"
	"go/ast"
	"go/build/constraint"
	"go/parser"
	"go/token"
	"path"
	"slices"
	"strings"
)

const module = "github.com/jewei/tinydash"

// Package is what the rules need to know about one Go package.
type Package struct {
	Path    string            // import path
	Imports []string          // imports of its non-test files
	Files   map[string][]byte // file name -> source, test files included
}

// layers lists, for each internal package (by prefix, after the module),
// the internal packages it may import. A package that matches no prefix
// fails, so a new package must be placed in a layer.
var layers = []struct {
	pkg   string
	allow []string
}{
	{"cmd/tinydash", []string{"internal/app"}},
	{"internal/archtest", nil},
	{"internal/core", nil},
	{"internal/match", []string{"internal/core"}},
	{"internal/feature/", []string{"internal/core", "internal/match"}},
	{"internal/settings", []string{"internal/core"}},
	{"internal/search", []string{"internal/core", "internal/match", "internal/feature/", "internal/settings"}},
	{"internal/store", []string{"internal/core", "internal/feature/"}},
	{"internal/platform", []string{"internal/core"}},
	{"internal/engine", []string{"internal/core", "internal/match", "internal/feature/", "internal/search", "internal/settings", "internal/store"}},
	{"internal/ui/", []string{"internal/core", "internal/search", "internal/settings", "internal/engine"}},
	{"internal/app", []string{"internal/"}},
}

// external lists every module package outside the standard library that the
// code may import, and which packages may import it. Anything else fails:
// a new dependency is a decision, made here.
var external = map[string][]string{
	"github.com/egoist/mygo":                []string{"internal/app", "internal/platform"},
	"github.com/egoist/mygo/ui":             []string{"internal/app", "internal/ui/"},
	"github.com/egoist/mygo/plugins/sqlite": []string{"internal/store"},
	"github.com/ebitengine/purego":          []string{"internal/platform"},
	"golang.org/x/sys/":                     []string{"internal/platform"},
	"golang.org/x/text/":                    []string{"internal/match", "internal/feature/"},
}

// pure packages get disk, network, clock, and randomness as arguments, so
// tests control them and a search never blocks on the OS.
var pure = []string{"internal/core", "internal/match", "internal/feature/", "internal/settings", "internal/search"}

var impureImports = []string{"os", "os/exec", "os/signal", "net", "net/", "syscall", "unsafe", "plugin", "crypto/rand", "math/rand", "math/rand/v2"}

// Clock reads that pure packages must not make.
var clockCalls = []string{"Now", "Since", "Until", "Sleep", "After", "Tick", "NewTimer", "NewTicker", "AfterFunc"}

var goosNames = []string{"aix", "android", "darwin", "dragonfly", "freebsd", "illumos", "ios", "js", "linux", "netbsd", "openbsd", "plan9", "solaris", "wasip1", "windows"}

// Check returns every rule the packages break, one line each.
func Check(pkgs []Package) []string {
	var out []string
	for _, p := range pkgs {
		rel, ok := strings.CutPrefix(p.Path, module+"/")
		if !ok {
			continue
		}
		allow, ok := layer(rel)
		if !ok {
			out = append(out, fmt.Sprintf("%s: not in any layer; add it to archtest.layers", rel))
			continue
		}
		for _, imp := range p.Imports {
			if dep, ok := strings.CutPrefix(imp, module+"/"); ok {
				if !matchesAny(dep, allow) {
					out = append(out, fmt.Sprintf("%s imports %s; its layer allows %v", rel, dep, allow))
				}
				continue
			}
			if isStd(imp) {
				if matchesAny(rel, pure) && matchesAny(imp, impureImports) {
					out = append(out, fmt.Sprintf("%s imports %s; pure packages take the OS, network, and randomness as arguments", rel, imp))
				}
				continue
			}
			users, known := externalUsers(imp)
			switch {
			case !known:
				out = append(out, fmt.Sprintf("%s imports %s, which is not in archtest.external", rel, imp))
			case !matchesAny(rel, users):
				out = append(out, fmt.Sprintf("%s imports %s; only %v may", rel, imp, users))
			}
		}
		out = append(out, checkFiles(rel, p.Files)...)
	}
	slices.Sort(out)
	return out
}

func checkFiles(rel string, files map[string][]byte) []string {
	var out []string
	platform := matchesAny(rel, []string{"internal/platform"})
	for name, src := range files {
		where := rel + "/" + name
		if !platform && namesGOOS(name) {
			out = append(out, where+": OS-specific file outside internal/platform")
		}
		fset := token.NewFileSet()
		f, err := parser.ParseFile(fset, name, src, parser.ParseComments)
		if err != nil {
			out = append(out, fmt.Sprintf("%s: %v", where, err))
			continue
		}
		if !platform && buildTagNamesGOOS(f) {
			out = append(out, where+": OS build tag outside internal/platform")
		}
		timeName, runtimeName := importName(f, "time"), importName(f, "runtime")
		isPure := matchesAny(rel, pure)
		ast.Inspect(f, func(n ast.Node) bool {
			sel, ok := n.(*ast.SelectorExpr)
			if !ok {
				return true
			}
			id, ok := sel.X.(*ast.Ident)
			if !ok {
				return true
			}
			pos := fset.Position(sel.Pos())
			if isPure && id.Name == timeName && slices.Contains(clockCalls, sel.Sel.Name) {
				out = append(out, fmt.Sprintf("%s:%d: time.%s in a pure package; take the time as an argument", where, pos.Line, sel.Sel.Name))
			}
			if !platform && name != "labels.go" && id.Name == runtimeName && sel.Sel.Name == "GOOS" {
				out = append(out, fmt.Sprintf("%s:%d: runtime.GOOS outside internal/platform and labels.go", where, pos.Line))
			}
			return true
		})
	}
	return out
}

func layer(rel string) ([]string, bool) {
	for _, l := range layers {
		if matches(rel, l.pkg) {
			return l.allow, true
		}
	}
	return nil, false
}

// matches reports whether path is pattern, or is under pattern when pattern
// ends in "/".
func matches(p, pattern string) bool {
	if strings.HasSuffix(pattern, "/") {
		return strings.HasPrefix(p, pattern) || p == strings.TrimSuffix(pattern, "/")
	}
	return p == pattern
}

func matchesAny(p string, patterns []string) bool {
	return slices.ContainsFunc(patterns, func(pattern string) bool { return matches(p, pattern) })
}

func externalUsers(imp string) ([]string, bool) {
	for dep, users := range external {
		if matches(imp, dep) {
			return users, true
		}
	}
	return nil, false
}

// isStd reports whether an import path is in the standard library: its first
// element has no dot.
func isStd(imp string) bool {
	first, _, _ := strings.Cut(imp, "/")
	return !strings.Contains(first, ".")
}

func namesGOOS(file string) bool {
	base := strings.TrimSuffix(strings.TrimSuffix(file, ".go"), "_test")
	parts := strings.Split(base, "_")
	for _, part := range parts[1:] {
		if slices.Contains(goosNames, part) {
			return true
		}
	}
	return false
}

func buildTagNamesGOOS(f *ast.File) bool {
	for _, group := range f.Comments {
		if group.Pos() > f.Package {
			break
		}
		for _, c := range group.List {
			expr, err := constraint.Parse(c.Text)
			if err != nil {
				continue
			}
			if namesOS(expr) {
				return true
			}
		}
	}
	return false
}

// namesOS walks the whole expression: Eval would stop early at || and &&.
func namesOS(e constraint.Expr) bool {
	switch e := e.(type) {
	case *constraint.TagExpr:
		return slices.Contains(goosNames, e.Tag) || e.Tag == "unix"
	case *constraint.NotExpr:
		return namesOS(e.X)
	case *constraint.AndExpr:
		return namesOS(e.X) || namesOS(e.Y)
	case *constraint.OrExpr:
		return namesOS(e.X) || namesOS(e.Y)
	}
	return false
}

// importName is the name a file uses for an import path, or "" without it.
func importName(f *ast.File, importPath string) string {
	for _, spec := range f.Imports {
		if strings.Trim(spec.Path.Value, `"`) != importPath {
			continue
		}
		if spec.Name != nil {
			return spec.Name.Name
		}
		return path.Base(importPath)
	}
	return ""
}
