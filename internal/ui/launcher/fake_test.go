package launcher

import (
	"errors"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/golden"
)

// fake is a Backend that records what the launcher asks.
type fake struct {
	drags    int
	results  func(query string, category core.Category) []core.Result
	settings ViewSettings
	// fail, if set, is the error an action ends with.
	fail func(core.Action) error
	// hold keeps the done functions of actions in waiting, for a test to
	// end them later.
	hold    bool
	waiting []func(error)

	searches  []searched
	runs      []ran
	paused    []bool
	pauseErr  error
	hidden    int
	enabled   int
	enableErr error

	// widgets is what a load of the widgets gives; widgetLoads counts
	// loads. holdWidgets keeps their done functions in loading.
	widgets     Widgets
	widgetLoads int
	holdWidgets bool
	loading     []func(Widgets)
	// warnings wait until the launcher asks for them.
	warnings []string

	// details are what a load of a result's details gives; detailLoads
	// are the results asked for. holdDetails keeps their done functions
	// in detailsWaiting.
	details        map[core.ID]Details
	detailLoads    []core.ID
	holdDetails    bool
	detailsWaiting []func()
}

type searched struct {
	query    string
	category core.Category
}

type ran struct {
	action core.Action
	from   core.ID
}

func newFake(results func(string, core.Category) []core.Result) *fake {
	return &fake{results: results, settings: ViewSettings{
		Tabs:             []core.Category{core.CategoryApps, core.CategoryFiles, core.CategoryClipboard, core.CategorySnippets, core.CategoryEmoji, core.CategorySystem},
		ClipboardHistory: true,
	}}
}

func (f *fake) Search(query string, category core.Category) []core.Result {
	f.searches = append(f.searches, searched{query, category})
	return f.results(query, category)
}

func (f *fake) Run(action core.Action, from core.ID, done func(error)) {
	f.runs = append(f.runs, ran{action, from})
	var err error
	if f.fail != nil {
		err = f.fail(action)
	}
	if f.hold {
		f.waiting = append(f.waiting, func(e error) { done(errors.Join(err, e)) })
		return
	}
	done(err)
}

// finish ends the oldest waiting action with err.
func (f *fake) finish(err error) {
	done := f.waiting[0]
	f.waiting = f.waiting[1:]
	done(err)
}

func (f *fake) Settings() ViewSettings     { return f.settings }
func (f *fake) FileIcon(string) *ui.Bitmap { return nil }
func (f *fake) Hide()                      { f.hidden++ }
func (f *fake) DragStarted()               { f.drags++ }
func (f *fake) PauseShortcuts(paused bool) error {
	f.paused = append(f.paused, paused)
	return f.pauseErr
}

func (f *fake) EnableClipboardHistory(done func(error)) {
	f.enabled++
	done(f.enableErr)
}

func (f *fake) Widgets(done func(Widgets)) {
	f.widgetLoads++
	if f.holdWidgets {
		f.loading = append(f.loading, done)
		return
	}
	done(f.widgets)
}

func (f *fake) Details(id core.ID, done func(Details, bool)) {
	f.detailLoads = append(f.detailLoads, id)
	d, ok := f.details[id]
	if f.holdDetails {
		f.detailsWaiting = append(f.detailsWaiting, func() { done(d, ok) })
		return
	}
	done(d, ok)
}

func (f *fake) Warnings() []string {
	w := f.warnings
	f.warnings = nil
	return w
}

func (f *fake) lastSearch() searched { return f.searches[len(f.searches)-1] }

var safariPath = golden.Abs("/Applications/Safari.app")

var safari = core.Result{
	ID: core.SourceApp.ID(safariPath), Kind: core.KindApp, Title: "Safari", Subtitle: "Application",
	Icon: core.SymbolIcon(core.SymbolApp),
	Actions: []core.ResultAction{
		core.Do("Open", core.Launch{Path: safariPath}),
		core.Do("Show in Finder", core.Reveal{Path: safariPath}),
	},
}

var restart = core.Result{
	ID: core.SourceSystem.ID("restart"), Kind: core.KindSystem, Title: "Restart", Subtitle: "System",
	Icon: core.SymbolIcon(core.SymbolRestart),
	Actions: []core.ResultAction{{
		Label: "Run", Action: core.System{Command: core.CommandRestart}, Confirm: "Restart the computer?",
	}},
}

var clip = core.Result{
	ID: core.SourceClip.ID("1"), Kind: core.KindClipboard, Title: "Copied text", Subtitle: "Clipboard",
	Icon: core.SymbolIcon(core.SymbolText),
	Actions: []core.ResultAction{
		core.Do("Copy", core.CopyClip{ID: 1}),
		core.Do("Pin", core.Pin{ID: core.SourceClip.ID("1")}),
		core.Do("Delete", core.DeleteClip{ID: 1}),
	},
}

// app is a copy of Safari with another title, path, and ID.
func app(title string) core.Result {
	r := safari
	path := golden.Abs("/Applications/" + title + ".app")
	r.ID, r.Title = core.SourceApp.ID(path), title
	r.Actions = []core.ResultAction{core.Do("Open", core.Launch{Path: path}), core.Do("Show in Finder", core.Reveal{Path: path})}
	return r
}

// withAction is r with one more action.
func withAction(r core.Result, ra core.ResultAction) core.Result {
	r.Actions = append(append([]core.ResultAction(nil), r.Actions...), ra)
	return r
}

func always(results ...core.Result) func(string, core.Category) []core.Result {
	return func(string, core.Category) []core.Result { return results }
}

// open starts a launcher over f in a tester, as the panel shows.
func open(t *testing.T, f *fake) (*Model, *ui.Tester) {
	t.Helper()
	m := New(f)
	tt := ui.NewTester(m.View, 760, 480)
	if !tt.Focused("Search") {
		t.Fatal("the search field has no focus as the launcher opens")
	}
	return m, tt
}

func click(t *testing.T, tt *ui.Tester, name string) {
	t.Helper()
	if err := tt.Click(name); err != nil {
		t.Fatalf("click %q: %v; texts %q", name, err, tt.Texts())
	}
}

func wantRuns(t *testing.T, f *fake, want ...ran) {
	t.Helper()
	if len(f.runs) != len(want) {
		t.Fatalf("ran %+v, want %+v", f.runs, want)
	}
	for i := range want {
		if f.runs[i] != want[i] {
			t.Errorf("run %d: %+v, want %+v", i, f.runs[i], want[i])
		}
	}
}

// selected is the accessible name of the selected result row.
func selectedRow(tt *ui.Tester, names ...string) string {
	for _, name := range names {
		if info, ok := tt.AccessByRole(ui.RoleOption, name); ok && info.Selected {
			return name
		}
	}
	return ""
}

// pressButton clicks the button with that name, where other text, such as
// the footer's, has the same words.
func pressButton(t *testing.T, tt *ui.Tester, name string) {
	t.Helper()
	b, ok := tt.AccessByRole(ui.RoleButton, name)
	if !ok {
		t.Fatalf("no button %q", name)
	}
	tt.ClickAt(b.Bounds.X+b.Bounds.W/2, b.Bounds.Y+b.Bounds.H/2)
}

// chooseItem clicks the item of the open actions menu with that label: the
// details pane has a button with the same words.
func chooseItem(t *testing.T, tt *ui.Tester, label string) {
	t.Helper()
	b, ok := tt.AccessByRole(ui.RoleOption, label)
	if !ok {
		t.Fatalf("no menu item %q", label)
	}
	tt.ClickAt(b.Bounds.X+b.Bounds.W/2, b.Bounds.Y+b.Bounds.H/2)
}
