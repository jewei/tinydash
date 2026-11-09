package archtest

import (
	"bytes"
	"encoding/json"
	"go/format"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

// TestModuleFollowsTheRules runs the rules over every package of the module.
func TestModuleFollowsTheRules(t *testing.T) {
	for _, problem := range Check(loadModule(t)) {
		t.Error(problem)
	}
}

// TestGoFilesAreFormatted fails on any file that gofmt would change, so a
// format check needs no separate command.
func TestGoFilesAreFormatted(t *testing.T) {
	for _, p := range loadModule(t) {
		for name, src := range p.Files {
			formatted, err := format.Source(src)
			if err != nil {
				t.Errorf("%s/%s: %v", p.Path, name, err)
			} else if !bytes.Equal(formatted, src) {
				t.Errorf("%s/%s is not gofmt-formatted; run gofmt -w", p.Path, name)
			}
		}
	}
}

// Each rule, shown failing on a package that breaks it.
func TestEachRuleCanFail(t *testing.T) {
	file := func(src string) map[string][]byte { return map[string][]byte{"x.go": []byte(src)} }
	cases := []struct {
		name string
		pkg  Package
		want string
	}{
		{"unknown package", Package{Path: module + "/internal/stuff"}, "not in any layer"},
		{"layer skipped", Package{Path: module + "/internal/feature/calc", Imports: []string{module + "/internal/store"}}, "imports internal/store"},
		{"features import each other", Package{Path: module + "/internal/feature/calc", Imports: []string{module + "/internal/feature/currency"}}, "imports internal/feature/currency"},
		{"pure package reads the disk", Package{Path: module + "/internal/feature/files", Imports: []string{"os"}}, "imports os; pure packages"},
		{"pure package reads randomness", Package{Path: module + "/internal/feature/password", Imports: []string{"crypto/rand"}}, "imports crypto/rand"},
		{"new dependency", Package{Path: module + "/internal/engine", Imports: []string{"github.com/some/lib"}}, "not in archtest.external"},
		{"sqlite outside store", Package{Path: module + "/internal/engine", Imports: []string{"github.com/egoist/mygo/plugins/sqlite"}}, "only [internal/store] may"},
		{"automation outside app", Package{Path: module + "/internal/engine", Imports: []string{"github.com/egoist/mygo/plugins/automation"}}, "only [internal/app] may"},
		{"views import each other", Package{Path: module + "/internal/ui/settings", Imports: []string{module + "/internal/ui/launcher"}}, "imports internal/ui/launcher"},
		{"kit imports a view", Package{Path: module + "/internal/ui/kit", Imports: []string{module + "/internal/ui/launcher"}}, "imports internal/ui/launcher"},
		{"kit imports the engine", Package{Path: module + "/internal/ui/kit", Imports: []string{module + "/internal/engine"}}, "imports internal/engine"},
		{"view imports the mygo root", Package{Path: module + "/internal/ui/launcher", Imports: []string{"github.com/egoist/mygo"}}, "only [internal/app internal/platform] may"},
		{"OS file name", Package{Path: module + "/internal/engine", Files: map[string][]byte{"monitor_darwin.go": []byte("package engine\n")}}, "OS-specific file"},
		{"OS build tag", Package{Path: module + "/internal/engine", Files: file("//go:build cgo || windows\n\npackage engine\n")}, "OS build tag"},
		{"clock in a pure package", Package{Path: module + "/internal/feature/datetime", Files: file("package datetime\n\nimport clock \"time\"\n\nvar now = clock.Now()\n")}, "time.Now in a pure package"},
		{"runtime.GOOS outside platform", Package{Path: module + "/internal/engine", Files: file("package engine\n\nimport \"runtime\"\n\nvar mac = runtime.GOOS == \"darwin\"\n")}, "runtime.GOOS outside"},
	}
	for _, c := range cases {
		got := strings.Join(Check([]Package{c.pkg}), "\n")
		if !strings.Contains(got, c.want) {
			t.Errorf("%s: want a problem with %q, got %q", c.name, c.want, got)
		}
	}
	// And the allowed shapes pass.
	ok := []Package{
		{Path: module + "/internal/feature/calc", Imports: []string{"strings", module + "/internal/core", module + "/internal/match"}, Files: file("package calc\n\nimport \"time\"\n\nvar d = time.Hour\n")},
		{Path: module + "/internal/platform", Imports: []string{"github.com/ebitengine/purego"}, Files: map[string][]byte{"clipboard_darwin.go": []byte("//go:build darwin\n\npackage platform\n")}},
		{Path: module + "/internal/ui/kit", Imports: []string{"github.com/egoist/mygo/ui", module + "/internal/core", module + "/internal/settings"}},
		{Path: module + "/internal/ui/settings", Imports: []string{module + "/internal/ui/kit"}},
		{Path: module + "/internal/ui/launcher", Imports: []string{"github.com/egoist/mygo/ui"}, Files: map[string][]byte{"labels.go": []byte("package launcher\n\nimport \"runtime\"\n\nvar mac = runtime.GOOS == \"darwin\"\n")}},
	}
	if problems := Check(ok); len(problems) > 0 {
		t.Errorf("allowed shapes failed: %q", problems)
	}
}

// loadModule lists every package with `go list`, with all of its files,
// whatever their build tags, and the imports of both its default and its
// e2e build: the e2e build links test-only packages the rules must see.
func loadModule(t *testing.T) []Package {
	t.Helper()
	cmd := exec.Command("go", "list", "-json", module+"/...")
	cmd.Env = append(os.Environ(), "GOFLAGS=")
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("go list: %v", err)
	}
	var pkgs []Package
	dec := json.NewDecoder(bytes.NewReader(out))
	for dec.More() {
		var listed struct {
			ImportPath, Dir                                              string
			Imports                                                      []string
			GoFiles, IgnoredGoFiles, TestGoFiles, XTestGoFiles, CgoFiles []string
		}
		if err := dec.Decode(&listed); err != nil {
			t.Fatal(err)
		}
		p := Package{Path: listed.ImportPath, Imports: listed.Imports, Files: map[string][]byte{}}
		for _, group := range [][]string{listed.GoFiles, listed.IgnoredGoFiles, listed.TestGoFiles, listed.XTestGoFiles, listed.CgoFiles} {
			for _, name := range group {
				src, err := os.ReadFile(filepath.Join(listed.Dir, name))
				if err != nil {
					t.Fatal(err)
				}
				p.Files[name] = src
			}
		}
		pkgs = append(pkgs, p)
	}
	e2e := e2eImports(t)
	for i := range pkgs {
		for _, imp := range e2e[pkgs[i].Path] {
			if !slices.Contains(pkgs[i].Imports, imp) {
				pkgs[i].Imports = append(pkgs[i].Imports, imp)
			}
		}
	}
	return pkgs
}

// e2eImports maps each package to its imports when built with -tags e2e.
func e2eImports(t *testing.T) map[string][]string {
	t.Helper()
	cmd := exec.Command("go", "list", "-tags", "e2e", "-f", "{{.ImportPath}}{{range .Imports}} {{.}}{{end}}", module+"/...")
	cmd.Env = append(os.Environ(), "GOFLAGS=")
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("go list -tags e2e: %v", err)
	}
	imports := map[string][]string{}
	for line := range strings.Lines(string(out)) {
		fields := strings.Fields(line)
		if len(fields) > 0 {
			imports[fields[0]] = fields[1:]
		}
	}
	return imports
}
