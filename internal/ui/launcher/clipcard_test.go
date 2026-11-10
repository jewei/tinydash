package launcher

import (
	"strings"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

var colorCard = ClipCard{
	ID:    "c1",
	Color: &ClipColor{Hex: "#2F6F5E", CopiedAs: "RGB", OnWhite: Contrast{"5.9:1", "AA"}, OnBlack: Contrast{"3.6:1", "AA large"}},
	Rows:  []CardRow{{"HEX", "#2F6F5E"}, {"RGB", "rgb(47, 111, 94)"}, {"HSL", "hsl(164, 41%, 31%)"}},
	Actions: []core.ResultAction{
		core.Do("Copy HEX", core.Copy{Text: "#2F6F5E"}),
		core.Do("Copy RGB", core.Copy{Text: "rgb(47, 111, 94)"}),
		core.Do("Copy HSL", core.Copy{Text: "hsl(164, 41%, 31%)"}),
	},
}

var timeCard = ClipCard{
	ID:   "t1",
	Time: &ClipTime{Raw: "1791354301", Relative: "6 hours ago"},
	Rows: []CardRow{{"Local", "Wed 7 Oct 2026, 14:25:01"}, {"UTC", "Wed 7 Oct 2026, 06:25:01 UTC"}, {"ISO", "2026-10-07T06:25:01Z"}},
	Actions: []core.ResultAction{
		core.Do("Copy Local Time", core.Copy{Text: "Wed 7 Oct 2026, 14:25:01"}),
		core.Do("Copy UTC Time", core.Copy{Text: "Wed 7 Oct 2026, 06:25:01 UTC"}),
		core.Do("Copy ISO 8601", core.Copy{Text: "2026-10-07T06:25:01Z"}),
	},
}

func jsonCard() ClipCard {
	return ClipCard{
		ID: "j1",
		JSON: &ClipJSON{
			Count: 1, Depth: 2, Bytes: 16, Keys: []JSONKey{{"a", "[2]"}}, Minified: `{"a":[1,true]}`,
			Lines: []JSONLine{
				{0, []JSONToken{{JSONPunctuation, "{"}}},
				{1, []JSONToken{{JSONKeyToken, `"a"`}, {JSONPunctuation, ": "}, {JSONPunctuation, "["}}},
				{2, []JSONToken{{JSONNumber, "1"}, {JSONPunctuation, ","}}},
				{2, []JSONToken{{JSONLiteral, "true"}}},
				{1, []JSONToken{{JSONPunctuation, "]"}}},
				{0, []JSONToken{{JSONPunctuation, "}"}}},
			},
			MoreLines: 1500,
		},
		Actions: []core.ResultAction{
			core.Do("Copy Pretty JSON", core.Copy{Text: "{\n  \"a\": [\n    1,\n    true\n  ]\n}"}),
			core.Do("Copy Minified JSON", core.Copy{Text: `{"a":[1,true]}`}),
		},
	}
}

func withCard(card ClipCard) *fake {
	f := newFake(always())
	f.widgets = Widgets{ClipCards: true, Card: &card}
	return f
}

func hasCard(tt *ui.Tester, kind string) bool {
	_, ok := tt.AccessByRole(ui.RoleGroup, kind+" on the clipboard")
	return ok
}

func TestShowsAColorCardAndCopiesItsForms(t *testing.T) {
	f := withCard(colorCard)
	_, tt := showLauncher(t, f)
	if !hasCard(tt, "Color") || !hasPane(tt) {
		t.Fatalf("no color card; texts %q", tt.Texts())
	}
	for _, text := range []string{"COLOR", "from RGB", "Copy HEX", "rgb(47, 111, 94)", "5.9:1 on white", "3.6:1 on black", "AA large"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	pressButton(t, tt, "Copy HSL")
	wantRuns(t, f, ran{colorCard.Actions[0].Action, ""}, ran{colorCard.Actions[2].Action, ""})
	if !tt.Focused("Search") {
		t.Error("the copy button kept the keyboard")
	}
}

func TestShowsAUnixTimeCard(t *testing.T) {
	f := withCard(timeCard)
	_, tt := showLauncher(t, f)
	for _, text := range []string{"UNIX TIME", "1791354301", "seconds", "6 hours ago", "Wed 7 Oct 2026, 06:25:01 UTC", "Copy Local Time"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	wantRuns(t, f, ran{timeCard.Actions[0].Action, ""})
}

// Mod+Shift+Enter opens the full JSON. Escape closes it before it hides the
// launcher, and the view closes as the launcher hides, so the widgets show
// again the next time.
func TestOpensAndClosesTheFullJSON(t *testing.T) {
	f := withCard(jsonCard())
	m, tt := showLauncher(t, f)
	for _, text := range []string{"JSON", "Object · 1 key · depth 2 · 16 bytes", "View Full JSON", `{"a":[1,true]}`, "Copy Pretty", "Copy Minified"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	if _, ok := tt.AccessByRole(ui.RoleDialog, "Clipboard JSON"); !ok || hasPane(tt) {
		t.Fatalf("no JSON view; texts %q", tt.Texts())
	}
	for _, text := range []string{"Clipboard JSON", "true", "1,500 more lines. Copy Pretty has all of them."} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	if len(f.runs) != 0 {
		t.Errorf("opening the view ran %+v", f.runs)
	}
	pressButton(t, tt, "Copy Pretty")
	wantRuns(t, f, ran{jsonCard().Actions[0].Action, ""})

	tt.Key(0, ui.KeyEscape)
	if !hasCard(tt, "JSON") || f.hidden != 0 {
		t.Fatalf("Escape did not go back to the card; hidden %d", f.hidden)
	}
	tt.Key(0, ui.KeyEscape)
	if f.hidden != 1 {
		t.Errorf("a second Escape did not hide; hidden %d", f.hidden)
	}

	click(t, tt, "View Full")
	m.Hidden()
	m.Show("")
	tt.Frame()
	if !hasCard(tt, "JSON") || !hasPane(tt) {
		t.Errorf("the view is still open after a hide; texts %q", tt.Texts())
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	click(t, tt, "Back to widgets")
	if !hasPane(tt) {
		t.Error("Back did not close the view")
	}
}

func hasJSONView(tt *ui.Tester) bool {
	_, ok := tt.AccessByRole(ui.RoleDialog, "Clipboard JSON")
	return ok
}

// Typing or another tab closes the view, as the Rust pane left the screen;
// so does a new card.
func TestTheJSONViewClosesWhenThePaneGoes(t *testing.T) {
	f := withCard(jsonCard())
	m, tt := showLauncher(t, f)
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	tt.Type("x")
	tt.Key(0, ui.KeyBackspace)
	if hasJSONView(tt) || !hasCard(tt, "JSON") {
		t.Error("the view came back after typing")
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	tt.Key(0, ui.KeyTab)
	tt.Key(ui.Shift, ui.KeyTab)
	if hasJSONView(tt) {
		t.Error("the view came back after another tab")
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	next := jsonCard()
	next.ID = "j2"
	f.widgets.Card = &next
	m.Stale()
	tt.Frame()
	if hasJSONView(tt) || !hasCard(tt, "JSON") {
		t.Error("a new card opened in the view")
	}
}

// A click on a line leaves the keyboard with the search field.
func TestAClickInTheJSONKeepsTheSearchFocused(t *testing.T) {
	_, tt := showLauncher(t, withCard(jsonCard()))
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	view, _ := tt.AccessByRole(ui.RoleDialog, "Clipboard JSON")
	tt.ClickAt(view.Bounds.X+view.Bounds.W/2, view.Bounds.Y+70)
	if !tt.Focused("Search") {
		t.Error("the search field lost the keyboard")
	}
}

// The menu's View Full JSON opens the card the menu showed, not a newer one.
func TestAMenuItemActsOnTheCardItShowed(t *testing.T) {
	f := withCard(jsonCard())
	m, tt := showLauncher(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	color := colorCard
	f.widgets.Card = &color
	m.Stale()
	chooseItem(t, tt, "View Full JSON")
	if len(f.runs) != 0 || hasJSONView(tt) || m.jsonFor != "" {
		t.Errorf("ran %+v; view %v; open for %q", f.runs, hasJSONView(tt), m.jsonFor)
	}
}

// Loading the widgets reads the clipboard, so a hidden panel loads them
// only as it shows.
func TestAHiddenPanelLoadsNoWidgets(t *testing.T) {
	f := withCard(colorCard)
	m, tt := showLauncher(t, f)
	m.Hidden()
	m.Stale()
	if f.widgetLoads != 1 {
		t.Errorf("loaded the widgets %d times, want once", f.widgetLoads)
	}
	m.Show("")
	m.Stale()
	tt.Frame()
	if f.widgetLoads != 3 {
		t.Errorf("loaded the widgets %d times, want 3", f.widgetLoads)
	}
}

// A hide drops the card and any load on its way, so the next show never
// acts on a copy the clipboard no longer holds.
func TestAHideDropsTheCard(t *testing.T) {
	f := withCard(colorCard)
	f.holdWidgets = true
	m, tt := showLauncher(t, f)
	f.loading[0](f.widgets)
	m.Hidden()
	m.Show("")
	tt.Frame()
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	if hasCard(tt, "Color") || len(f.runs) != 0 {
		t.Errorf("the old card shows or ran before its load: %+v", f.runs)
	}
	// A load from before the hide lands late: it is dropped.
	m.Hidden()
	m.Show("")
	f.loading[1](f.widgets)
	tt.Frame()
	if hasCard(tt, "Color") {
		t.Error("a load from before the hide showed")
	}
	f.loading[2](f.widgets)
	tt.Frame()
	if !hasCard(tt, "Color") {
		t.Error("the latest load did not show")
	}
}

// After a failed command shows the panel again, the widgets load again.
func TestShownAgainLoadsTheWidgets(t *testing.T) {
	f := withCard(colorCard)
	m, tt := showLauncher(t, f)
	m.Hidden()
	m.ShownAgain()
	m.Stale()
	tt.Frame()
	if f.widgetLoads != 3 || !hasCard(tt, "Color") {
		t.Errorf("loads %d; texts %q", f.widgetLoads, tt.Texts())
	}
}

// JSON too deep to copy pretty has no Copy Pretty.
func TestJSONWithoutPrettyCopy(t *testing.T) {
	card := jsonCard()
	card.Actions = card.Actions[1:]
	_, tt := showLauncher(t, withCard(card))
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	if !tt.HasText("1,500 more lines.") || tt.HasText("Copy Pretty") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// A dismissed card stays away until the clipboard holds a new copy, also
// after a search hid the pane.
func TestDismissesACardUntilANewCopy(t *testing.T) {
	f := withCard(colorCard)
	m, tt := showLauncher(t, f)
	click(t, tt, "Dismiss")
	if hasCard(tt, "Color") || !tt.HasText("Copy a color, a Unix time, or JSON to see it here.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	if !tt.Focused("Search") {
		t.Error("Dismiss kept the keyboard")
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	tt.Type("x")
	tt.Key(0, ui.KeyBackspace)
	m.Show("")
	tt.Frame()
	if hasCard(tt, "Color") || len(f.runs) != 0 {
		t.Errorf("the dismissed card came back or ran: %+v", f.runs)
	}
	next := timeCard
	f.widgets.Card = &next
	m.Show("")
	tt.Frame()
	if !hasCard(tt, "Unix time") {
		t.Errorf("a new copy has no card; texts %q", tt.Texts())
	}
}

func TestOnlyClipboardCardsOnShowsAHint(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{ClipCards: true}
	_, tt := showLauncher(t, f)
	if !hasPane(tt) || !tt.HasText("Copy a color, a Unix time, or JSON to see it here.") {
		t.Errorf("texts %q", tt.Texts())
	}
	tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
	if len(f.runs) != 0 {
		t.Errorf("ran %+v without a card", f.runs)
	}
}

// The menu lists the card's actions, the main one with the card's keys.
func TestTheMenuListsTheCardsActions(t *testing.T) {
	f := withCard(jsonCard())
	m, tt := showLauncher(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	var labels []string
	for _, item := range m.menu.items {
		labels = append(labels, item.label+" "+strings.Join(item.keys, ""))
	}
	if !strings.HasPrefix(strings.Join(labels, "|"), "View Full JSON "+modKey+"⇧↩|Copy Pretty JSON |Copy Minified JSON |") {
		t.Errorf("menu %q", labels)
	}
	chooseItem(t, tt, "View Full JSON")
	if _, ok := tt.AccessByRole(ui.RoleDialog, "Clipboard JSON"); !ok {
		t.Error("the menu did not open the view")
	}

	f = withCard(colorCard)
	m, tt = showLauncher(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	if item := m.menu.items[0]; item.label != "Copy HEX" || strings.Join(item.keys, "") != modKey+"⇧↩" {
		t.Errorf("first item %+v", item)
	}
	tt.Type("hsl")
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{colorCard.Actions[2].Action, ""})
}

// Long text stays inside its card.
func TestLongTextStaysInsideItsCard(t *testing.T) {
	long := strings.Repeat("abcdefghij", 40)
	card := jsonCard()
	card.JSON.Minified = long
	card.JSON.Keys = []JSONKey{{long, ""}, {"b", "{1}"}}
	f := withCard(card)
	_, tt := showLauncher(t, f)
	group, ok := tt.AccessByRole(ui.RoleGroup, "JSON on the clipboard")
	pane, _ := tt.AccessByRole(ui.RoleRegion, "Widgets")
	if !ok || group.Bounds.X+group.Bounds.W > pane.Bounds.X+pane.Bounds.W {
		t.Errorf("card %+v leaves the pane %+v", group.Bounds, pane.Bounds)
	}

	time := timeCard
	time.Rows = []CardRow{{"Local", long}}
	f = withCard(time)
	_, tt = showLauncher(t, f)
	group, _ = tt.AccessByRole(ui.RoleGroup, "Unix time on the clipboard")
	copyButton, ok := tt.AccessByRole(ui.RoleButton, "Copy Local")
	if !ok || copyButton.Bounds.X+copyButton.Bounds.W > group.Bounds.X+group.Bounds.W {
		t.Errorf("the copy button %+v left the card %+v", copyButton.Bounds, group.Bounds)
	}
}
