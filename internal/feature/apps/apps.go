// Package apps indexes and ranks installed applications. The platform finds
// them; this package only orders and searches the list it is given.
package apps

import (
	"cmp"
	"path/filepath"
	"slices"
	"strings"

	"golang.org/x/text/unicode/norm"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// MaxApps is the most apps the index holds; more are cut off.
const MaxApps = 5_000

// App is one installed application.
type App struct {
	Name string
	// Path is the bundle, shortcut, or desktop entry. Launching goes
	// through the platform.
	Path string
	// Aliases are other names that should find the app, such as its
	// executable.
	Aliases []string
	// Description says what the app is for, such as "Web browser", when the
	// OS can say; it is empty otherwise.
	Description string
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

// Index is the apps found, sorted by name.
type Index struct {
	apps []App
	// Truncated: the platform found more than MaxApps.
	Truncated bool
}

// NewIndex keeps one app per path, up to MaxApps in the order found, and
// sorts them by name. Names and aliases go into NFC, the form the matcher
// gives the query.
func NewIndex(apps []App) *Index {
	seen := map[string]bool{}
	var kept []App
	for _, app := range apps {
		if seen[app.Path] {
			continue
		}
		seen[app.Path] = true
		app.Name = norm.NFC.String(app.Name)
		aliases := make([]string, len(app.Aliases))
		for i, alias := range app.Aliases {
			aliases[i] = norm.NFC.String(alias)
		}
		app.Aliases = aliases
		kept = append(kept, app)
	}
	x := &Index{Truncated: len(kept) > MaxApps}
	x.apps = kept[:min(len(kept), MaxApps)]
	slices.SortStableFunc(x.apps, func(a, b App) int {
		return cmp.Or(cmp.Compare(strings.ToLower(a.Name), strings.ToLower(b.Name)), cmp.Compare(a.Path, b.Path))
	})
	return x
}

// Len is the number of apps.
func (x *Index) Len() int { return len(x.apps) }

// Contains reports whether an app has this path.
func (x *Index) Contains(path string) bool {
	return slices.ContainsFunc(x.apps, func(app App) bool { return app.Path == path })
}

// Search ranks the apps whose name or alias matches; the best limit come
// first.
func (x *Index) Search(m *match.Matcher, ctx Context, limit int) []core.Scored {
	var hits []match.Hit[*App]
	for i := range x.apps {
		app := &x.apps[i]
		if score, ok := m.Best(app.Name, app.Aliases...); ok {
			hits = append(hits, match.Hit[*App]{Score: score + ctx.boost(id(app)), Item: app})
		}
	}
	top := match.Top(hits, limit)
	out := make([]core.Scored, len(top))
	for i, hit := range top {
		out[i] = core.Scored{Score: hit.Score, Result: result(hit.Item, ctx)}
	}
	return out
}

// Browse lists every app, most used first, then by name.
func (x *Index) Browse(ctx Context) []core.Result {
	apps := make([]*App, len(x.apps))
	for i := range x.apps {
		apps[i] = &x.apps[i]
	}
	slices.SortStableFunc(apps, func(a, b *App) int { return cmp.Compare(ctx.boost(id(b)), ctx.boost(id(a))) })
	out := make([]core.Result, len(apps))
	for i, app := range apps {
		out[i] = result(app, ctx)
	}
	return out
}

// Get is the result of the app at path.
func (x *Index) Get(path string, ctx Context) (core.Result, bool) {
	for i := range x.apps {
		if x.apps[i].Path == path {
			return result(&x.apps[i], ctx), true
		}
	}
	return core.Result{}, false
}

func id(app *App) core.ID { return core.SourceApp.ID(app.Path) }

func result(app *App, ctx Context) core.Result {
	resultID := id(app)
	// Its folder when the OS does not say; the details show the path.
	subtitle := app.Description
	if subtitle == "" {
		subtitle = displayPath(parent(app.Path), ctx.Home)
	}
	icon := core.SymbolIcon(core.SymbolApp)
	if nativeIcons {
		icon = core.FileIcon(app.Path)
	}
	pin := core.Do("Pin", core.Pin{ID: resultID})
	pinned := ctx.pinned(resultID)
	if pinned {
		pin = core.Do("Unpin", core.Unpin{ID: resultID})
	}
	return core.Result{
		ID:       resultID,
		Kind:     core.KindApp,
		Title:    app.Name,
		Subtitle: subtitle,
		Icon:     icon,
		Actions: []core.ResultAction{
			core.Do("Open", core.Launch{Path: app.Path}),
			core.Do("Show in "+fileManager, core.Reveal{Path: app.Path}),
			core.Do("Copy Path", core.Copy{Text: app.Path}),
			pin,
		},
		Pinned: pinned,
	}
}

// parent is the folder that holds path, or "" for a root or a bare name.
func parent(path string) string {
	dir := filepath.Dir(path)
	if dir == path || dir == "." {
		return ""
	}
	return dir
}

// displayPath shows a path in NFC, with the home folder as "~". The files
// package has the same rule; features may not import each other.
func displayPath(path, home string) string {
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
