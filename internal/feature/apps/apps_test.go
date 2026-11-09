package apps

import (
	"fmt"
	"slices"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

type personal struct {
	boost map[core.ID]uint32
	pins  map[core.ID]bool
}

func (p personal) Boost(id core.ID) uint32 { return p.boost[id] }
func (p personal) Pinned(id core.ID) bool  { return p.pins[id] }

func app(name, path string) App { return App{Name: name, Path: path} }

func names(x *Index) []string {
	var out []string
	for _, a := range x.apps {
		out = append(out, a.Name)
	}
	return out
}

func TestIndexSortsByNameAndDropsDuplicatePaths(t *testing.T) {
	x := NewIndex([]App{app("zed", "/z"), app("Arc", "/a"), app("Arc copy", "/a")})
	if got := names(x); !slices.Equal(got, []string{"Arc", "zed"}) {
		t.Errorf("names %q", got)
	}
}

func TestIndexKeepsTheFirstAppsFoundUpToTheLimit(t *testing.T) {
	var apps []App
	for n := 0; n <= MaxApps; n++ {
		apps = append(apps, app(fmt.Sprintf("App %d", n), fmt.Sprintf("/%d", n)))
	}
	x := NewIndex(apps)
	if x.Len() != MaxApps || !x.Truncated || x.Contains(fmt.Sprintf("/%d", MaxApps)) {
		t.Errorf("len %d, truncated %v", x.Len(), x.Truncated)
	}
	// A duplicate path does not count.
	apps = append(apps[:MaxApps], app("Copy", "/0"))
	if NewIndex(apps).Truncated {
		t.Error("a duplicate path counted toward the limit")
	}
}

func TestAliasesMatchInEitherUnicodeForm(t *testing.T) {
	writer := App{Name: "Writer", Path: "/w", Aliases: []string{"Résumé"}}
	x := NewIndex([]App{writer})
	if hits := x.Search(match.New("résumé"), Context{}, 10); len(hits) != 1 {
		t.Errorf("%d hits", len(hits))
	}
}

func TestUsageOrdersEqualMatches(t *testing.T) {
	x := NewIndex([]App{app("Notes", "/n"), app("Notion", "/o")})
	ctx := Context{Personal: personal{boost: map[core.ID]uint32{"app:/o": 50}}}
	if hits := x.Search(match.New("no"), ctx, 10); hits[0].Result.Title != "Notion" {
		t.Errorf("search first %q", hits[0].Result.Title)
	}
	if first := x.Browse(ctx)[0].Title; first != "Notion" {
		t.Errorf("browse first %q", first)
	}
}

func TestSaysWhatAnAppIsForOrWhereItIs(t *testing.T) {
	described := App{Name: "Safari", Path: "/Applications/Safari.app", Description: "Web browser"}
	if got := result(&described, Context{}).Subtitle; got != "Web browser" {
		t.Errorf("subtitle %q", got)
	}
	plain := app("Tool", "/Applications/Tool.app")
	if got := result(&plain, Context{}).Subtitle; got != "/Applications" {
		t.Errorf("subtitle %q", got)
	}
	home := app("Mine", "/Users/me/Applications/Mine.app")
	if got := result(&home, Context{Home: "/Users/me"}).Subtitle; got != "~/Applications" {
		t.Errorf("subtitle %q", got)
	}
}

// The shape the Rust app gave, from testdata/golden/search.jsonl on macOS.
func TestResultActionsAndPins(t *testing.T) {
	x := NewIndex([]App{{Name: "Safari", Path: "/Applications/Safari.app", Description: "Web browser"}})
	ctx := Context{Personal: personal{pins: map[core.ID]bool{"app:/Applications/Safari.app": true}}}
	r, ok := x.Get("/Applications/Safari.app", ctx)
	if !ok || r.ID != "app:/Applications/Safari.app" || r.Kind != core.KindApp || !r.Pinned {
		t.Fatalf("%+v", r)
	}
	var labels []string
	for _, a := range r.Actions {
		labels = append(labels, a.Label)
	}
	want := []string{"Open", "Show in " + fileManager, "Copy Path", "Unpin"}
	if !slices.Equal(labels, want) {
		t.Errorf("labels %q, want %q", labels, want)
	}
	if r.Actions[0].Action != (core.Launch{Path: "/Applications/Safari.app"}) {
		t.Errorf("first action %#v", r.Actions[0].Action)
	}
	if _, ok := x.Get("/nowhere", ctx); ok {
		t.Error("found a missing app")
	}
}
