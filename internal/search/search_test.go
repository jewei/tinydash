package search

import (
	"math/rand/v2"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/calc"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/feature/emoji"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/settings"
)

// empty is a world with only the built-in sources: emoji and System.
func empty() *Snapshot {
	return &Snapshot{
		Settings:     settings.Default(),
		Apps:         apps.NewIndex(nil),
		Files:        files.Scan(mapDisk{}, files.Options{}),
		Clipboard:    clipboard.NewHistory(nil),
		Library:      library.NewIndex(nil),
		Calculations: calc.NewHistory(nil),
		Emoji:        emoji.NewIndex(),
		Now:          time.Date(2026, 11, 9, 10, 30, 0, 0, time.UTC),
		Random:       rand.NewChaCha8([32]byte{1}),
		AppsReady:    true,
		FilesReady:   true,
	}
}

func ids(results []core.Result) []core.ID {
	out := make([]core.ID, len(results))
	for i, r := range results {
		out[i] = r.ID
	}
	return out
}

func titles(results []core.Result) []string {
	out := make([]string, len(results))
	for i, r := range results {
		out[i] = r.Title
	}
	return out
}

func quicklink(id int64, name, keyword, text string) library.Item {
	return library.Item{ID: id, Kind: library.KindQuicklink, Name: name, Keyword: keyword, Text: text}
}

func withTabs(s *Snapshot, inAll func(core.Category) bool) {
	tabs := slices.Clone(s.Settings.Tabs)
	for i := range tabs {
		tabs[i].InAll = inAll(tabs[i].Category)
	}
	s.Settings.Tabs = tabs
}

func TestACurrencyQueryAsksForRatesWhileTheyAreOff(t *testing.T) {
	s := empty()
	for _, query := range []string{"100 usd to eur", "=100 usd to eur"} {
		results := Search(s, query, core.CategoryAll)
		if results[0].Title != "Currency rates are off" || results[0].Actions[0].Action != (core.TurnOnCurrencyRates{}) {
			t.Errorf("%s: %q", query, titles(results))
		}
	}
	if slices.Contains(titles(Search(s, "12 * 8", core.CategoryAll)), "Currency rates are off") {
		t.Error("plain math asks for rates")
	}
}

func TestRatesThatAreOnConvert(t *testing.T) {
	s := empty()
	s.Settings.CurrencyRatesEnabled = true
	s.Rates = &currency.Rates{Date: "2026-11-06", Table: map[string]float64{"USD": 1.16}}
	results := Search(s, "116 usd to eur", core.CategoryAll)
	if results[0].Kind != core.KindCalculation || !strings.Contains(results[0].Subtitle, "ECB rates from 2026-11-06") {
		t.Errorf("%+v", results[0])
	}
	// While rates are on but not downloaded yet, nil reaches the calculator
	// as no rates at all.
	s.Rates = nil
	for _, r := range Search(s, "116 usd to eur", core.CategoryAll) {
		if r.Kind == core.KindCalculation {
			t.Errorf("an answer without rates: %+v", r)
		}
	}
}

func TestColorsAndUnixTimesAreAnswersInAll(t *testing.T) {
	s := empty()
	answers := func(query string) []core.Result {
		return slices.DeleteFunc(Search(s, query, core.CategoryAll), func(r core.Result) bool { return r.Kind == core.KindWebSearch })
	}
	if got := answers("#2f6f5e"); got[0].Kind != core.KindColor || got[0].Title != "#2F6F5E" {
		t.Errorf("color: %q", titles(got))
	}
	if got := answers("1791354301"); len(got) != 1 || got[0].Kind != core.KindDateTime {
		t.Errorf("unix time: %q", titles(got))
	}
}

func TestAnISODateAnswersWithItsWeekdayFirst(t *testing.T) {
	first := Search(empty(), "2026-10-17", core.CategoryAll)[0]
	if first.Kind != core.KindDateTime || first.Title != "Saturday, 17 October 2026" {
		t.Errorf("%+v", first)
	}
}

func TestASourceKeptOutOfAllStaysInItsTab(t *testing.T) {
	s := empty()
	withTabs(s, func(c core.Category) bool { return c != core.CategoryEmoji })
	const rocket core.ID = "emoji:🚀"
	hasEmoji := slices.ContainsFunc(ids(Search(s, "rocket", core.CategoryAll)), func(id core.ID) bool { return strings.HasPrefix(string(id), "emoji:") })
	if hasEmoji || !slices.Contains(ids(Search(s, "rocket", core.CategoryEmoji)), rocket) {
		t.Error("emoji searched in All, or not in their tab")
	}
	// Not suggested either, but a pin is the user's own choice.
	s.Usage = NewUsage(map[core.ID]core.Use{rocket: {Count: 5}})
	if slices.Contains(ids(Search(s, "", core.CategoryAll)), rocket) {
		t.Error("suggested from a source out of All")
	}
	s.Pins = NewPins([]core.ID{rocket})
	if got := ids(Search(s, "", core.CategoryAll)); got[0] != rocket {
		t.Errorf("the pin does not head All: %q", got)
	}
	// Its alias works in its tab only, too.
	s.Aliases = NewAliases(map[core.ID]string{rocket: "rk"})
	if slices.Contains(ids(Search(s, "rk", core.CategoryAll)), rocket) || ids(Search(s, "rk", core.CategoryEmoji))[0] != rocket {
		t.Error("the alias works in All or not in its tab")
	}
}

func TestQuicklinksStayOutOfAllWithTheirTab(t *testing.T) {
	s := empty()
	withTabs(s, func(c core.Category) bool { return c != core.CategorySnippets })
	s.Library = library.NewIndex([]library.Item{quicklink(1, "Jira", "", "https://jira.test/{query}")})
	if slices.Contains(titles(Search(s, "rust", core.CategoryAll)), "Jira: rust") {
		t.Error("a quicklink fallback from a tab out of All")
	}
}

func TestAKeywordWithTextAnswersInTheSnippetsTab(t *testing.T) {
	s := empty()
	s.Library = library.NewIndex([]library.Item{quicklink(1, "Jira", "jira", "https://jira.test/browse/{query}")})
	if got := titles(Search(s, "jira rust", core.CategorySnippets)); !slices.Equal(got, []string{"Jira: rust"}) {
		t.Errorf("%q", got)
	}
	// Its name still matches, but that row would be the same item.
	results := Search(s, "jira ji", core.CategorySnippets)
	if !slices.Equal(titles(results), []string{"Jira: ji"}) {
		t.Errorf("%q", titles(results))
	}
	s.Hidden = NewHidden(results[0].ID)
	if got := Search(s, "jira rust", core.CategorySnippets); len(got) != 0 {
		t.Errorf("a hidden item answers: %q", titles(got))
	}
}

func TestTheStartScreenSkipsPinnedClips(t *testing.T) {
	s := empty()
	s.Settings.ClipboardHistoryEnabled = true
	s.Clipboard = clipboard.NewHistory([]clipboard.Entry{{ID: 7, Kind: clipboard.KindText, Title: "meeting notes", Text: "meeting notes"}})
	command := Search(s, "", core.CategorySystem)[0].ID
	s.Pins = NewPins([]core.ID{"clip:7", command})
	start := ids(Search(s, "", core.CategoryAll))
	if start[0] != command || slices.Contains(start, "clip:7") {
		t.Errorf("start screen %q", start)
	}
	if got := ids(Search(s, "", core.CategoryClipboard)); got[0] != "clip:7" {
		t.Errorf("clipboard tab %q", got)
	}
	if !slices.Contains(ids(Search(s, "meeting", core.CategoryAll)), "clip:7") {
		t.Error("a search does not find the pinned clip")
	}
}

func TestClipboardHistoryOffHidesEveryEntry(t *testing.T) {
	s := empty()
	s.Clipboard = clipboard.NewHistory([]clipboard.Entry{{ID: 7, Kind: clipboard.KindText, Title: "meeting notes", Text: "meeting notes"}})
	s.Pins = NewPins([]core.ID{"clip:7"})
	for _, category := range []core.Category{core.CategoryAll, core.CategoryClipboard} {
		for _, query := range []string{"", "meeting"} {
			if slices.Contains(ids(Search(s, query, category)), "clip:7") {
				t.Errorf("%s %q shows an entry while history is off", category, query)
			}
		}
	}
	if _, ok := Resolve(s, "clip:7"); ok {
		t.Error("Resolve finds an entry while history is off")
	}
}

func TestAKeywordSearchLeavesNoSlotForTheFallback(t *testing.T) {
	s := empty()
	keyword := Search(s, "g a", core.CategoryAll)
	if len(keyword) != AllLimit || keyword[0].Kind != core.KindWebSearch {
		t.Errorf("g a: %d rows, first %s", len(keyword), keyword[0].Kind)
	}
	plain := Search(s, "a", core.CategoryAll)
	if len(plain) != AllLimit || plain[AllLimit-1].Kind != core.KindWebSearch {
		t.Errorf("a: %d rows, last %s", len(plain), plain[len(plain)-1].Kind)
	}
}

func TestAQuicklinkShowsOnceWhenItsKeywordAnswers(t *testing.T) {
	s := empty()
	s.Library = library.NewIndex([]library.Item{quicklink(1, "Jira", "jira", "https://jira.test/browse/{query}")})
	results := Search(s, "jira a", core.CategoryAll)
	if results[0].Title != "Jira: a" {
		t.Errorf("first %q", results[0].Title)
	}
	unique := slices.Compact(slices.Sorted(slices.Values(ids(results))))
	if len(unique) != len(results) {
		t.Errorf("two rows share an ID: %q", ids(results))
	}
}

func TestAQuicklinkKeywordGoesBeforeAWebKeyword(t *testing.T) {
	s := empty()
	s.Library = library.NewIndex([]library.Item{quicklink(1, "Work wiki", "w", "https://wiki.test/search?q={query}")})
	results := Search(s, "w vacation policy", core.CategoryAll)
	if results[0].Title != "Work wiki: vacation policy" || results[1].Kind != core.KindWebSearch {
		t.Errorf("%q", titles(results))
	}
}

func TestAnAliasLiftsItsResultOnceInAllAndInItsTab(t *testing.T) {
	s := empty()
	s.Aliases = NewAliases(map[core.ID]string{"system:sleep": "lo"})
	// "lo" is the alias of Sleep, and Lock Screen and Log Out start with it.
	for _, category := range []core.Category{core.CategoryAll, core.CategorySystem} {
		found := ids(Search(s, "lo", category))
		sleeps := 0
		for _, id := range found {
			if id == "system:sleep" {
				sleeps++
			}
		}
		if found[0] != "system:sleep" || sleeps != 1 {
			t.Errorf("%s: %q", category, found)
		}
	}
	if slices.Contains(ids(Search(s, "lo", core.CategoryApps)), "system:sleep") {
		t.Error("the alias works in another tab")
	}
	sleep := Search(s, "sleep", core.CategorySystem)[0]
	if !slices.ContainsFunc(sleep.Actions, func(a core.ResultAction) bool { return a.Label == "Change Alias “lo”…" }) {
		t.Errorf("labels of %+v", sleep.Actions)
	}
	s.Hidden = NewHidden("system:sleep")
	if slices.Contains(ids(Search(s, "lo", core.CategoryAll)), "system:sleep") {
		t.Error("a hidden result found by its alias")
	}
}

func TestAHotkeyIsOfferedOnlyWhereNothingAsksFirst(t *testing.T) {
	s := empty()
	actions := func(query string) []core.Action {
		var out []core.Action
		for _, a := range Search(s, query, core.CategorySystem)[0].Actions {
			out = append(out, a.Action)
		}
		return out
	}
	if !slices.Contains(actions("lock screen"), core.Action(core.SetHotkey{ID: "system:lock"})) {
		t.Error("no Set Hotkey on Lock Screen")
	}
	s.Hotkeys = NewHotkeys(map[core.ID]string{"system:lock": "Alt+KeyL"})
	if !slices.Contains(actions("lock screen"), core.Action(core.SetHotkey{ID: "system:lock", Shortcut: "Alt+KeyL"})) {
		t.Error("no Change Hotkey on Lock Screen")
	}
	// Restart asks first, which a hotkey pressed in another app cannot.
	if slices.ContainsFunc(actions("restart"), func(a core.Action) bool { _, ok := a.(core.SetHotkey); return ok }) {
		t.Error("a hotkey offered for Restart")
	}
}

func TestQuicklinksThatTakeTheQueryFollowTheWebSearch(t *testing.T) {
	s := empty()
	var items []library.Item
	for i, link := range [][2]string{{"Jira", "jira"}, {"Maps", ""}, {"Wiki", ""}, {"Docs", ""}} {
		items = append(items, quicklink(int64(i+1), link[0], link[1], "https://"+link[0]+".test/{query}"))
	}
	s.Library = library.NewIndex(items)
	found := titles(Search(s, "rust", core.CategoryAll))
	if tail := found[len(found)-4:]; !slices.Equal(tail, []string{"Search Google for “rust”", "Docs: rust", "Jira: rust", "Maps: rust"}) {
		t.Errorf("tail %q", tail)
	}
	// A keyword chose where the text goes.
	if slices.ContainsFunc(titles(Search(s, "g rust", core.CategoryAll)), func(title string) bool { return strings.HasSuffix(title, ": rust") }) {
		t.Error("fallbacks after a web keyword")
	}
	if slices.Contains(titles(Search(s, "jira rust", core.CategoryAll)), "Maps: jira rust") {
		t.Error("fallbacks after a quicklink keyword")
	}
	// A link that matched by name shows once, as itself.
	wiki := titles(Search(s, "wiki", core.CategoryAll))
	if !slices.Contains(wiki, "Wiki") || slices.Contains(wiki, "Wiki: wiki") {
		t.Errorf("wiki %q", wiki)
	}
	// Rows with typed text leave aliases to the item's own row.
	for _, r := range Search(s, "rust", core.CategoryAll) {
		if r.Title == "Jira: rust" && (len(r.Actions) != 1 || r.Actions[0].Label != "Open Quicklink") {
			t.Errorf("actions of Jira: rust: %+v", r.Actions)
		}
	}
	docs := Search(s, "", core.CategorySnippets)[0].ID
	s.Hidden = NewHidden(docs)
	rust := titles(Search(s, "rust", core.CategoryAll))
	if slices.Contains(rust, "Docs: rust") || !slices.Contains(rust, "Wiki: rust") {
		t.Errorf("with Docs hidden: %q", rust)
	}
}

func TestAFullListKeepsRoomForEveryQuicklinkItOffers(t *testing.T) {
	s := empty()
	// "a" matches more than a full list of emoji. "A Wiki" shows as a name
	// match; "Maps" matches by name too, below the cut, so it is offered as
	// a fallback instead.
	var items []library.Item
	for i, name := range []string{"A Wiki", "Docs", "Maps", "Notes"} {
		items = append(items, quicklink(int64(i+1), name, "", "https://x.test/{query}"))
	}
	s.Library = library.NewIndex(items)
	results := titles(Search(s, "a", core.CategoryAll))
	if len(results) != AllLimit || !slices.Equal(results[AllLimit-3:], []string{"Docs: a", "Maps: a", "Notes: a"}) {
		t.Errorf("%d rows, tail %q", len(results), results[max(len(results)-3, 0):])
	}
	if !slices.Contains(results, "A Wiki") || slices.Contains(results, "A Wiki: a") {
		t.Errorf("%q", results)
	}
}

func TestAHiddenResultLeavesItsSlotToTheNextOne(t *testing.T) {
	s := empty()
	before := ids(Search(s, "a", core.CategoryAll))
	if len(before) != AllLimit {
		t.Fatalf("%d rows", len(before))
	}
	s.Hidden = NewHidden(before[0])
	after := ids(Search(s, "a", core.CategoryAll))
	if len(after) != AllLimit || slices.Contains(after, before[0]) || !slices.Contains(after, before[AllLimit-2]) {
		t.Errorf("after hiding the first: %q", after)
	}
	// Hiding every row the user can hide makes each source reach past its
	// first AllLimit matches to fill the list again.
	var hideable []core.ID
	for _, id := range before {
		if traits, ok := id.Traits(); ok && traits.Hideable {
			hideable = append(hideable, id)
		}
	}
	s.Hidden = NewHidden(hideable...)
	after = ids(Search(s, "a", core.CategoryAll))
	if len(after) != AllLimit || slices.ContainsFunc(after, func(id core.ID) bool { return slices.Contains(hideable, id) }) {
		t.Errorf("after hiding all: %q", after)
	}
	// The emoji search and the empty Emoji tab each cut a long list.
	for _, query := range []string{"a", ""} {
		s.Hidden = NewHidden()
		shown := ids(Search(s, query, core.CategoryEmoji))
		if len(shown) != CategoryLimit {
			t.Fatalf("%q: %d rows", query, len(shown))
		}
		s.Hidden = NewHidden(shown[0])
		after := ids(Search(s, query, core.CategoryEmoji))
		if len(after) != CategoryLimit || slices.Contains(after, shown[0]) {
			t.Errorf("%q: %d rows after hiding one", query, len(after))
		}
	}
}

func TestEachBuiltInKeywordExampleGetsAnAnswer(t *testing.T) {
	s := empty()
	v := newView(s)
	for _, example := range library.Examples() {
		if _, ok := s.Calculations.Answer(example); !ok && len(answers(v, example)) == 0 {
			t.Errorf("%q gets no answer", example)
		}
	}
}

func TestAQuestionMarkListsKeywordsInAllOnly(t *testing.T) {
	s := empty()
	results := Search(s, "?", core.CategoryAll)
	if len(results) == 0 {
		t.Fatal("no keywords")
	}
	for _, r := range results {
		// No Add Alias or Hide: a keyword row is not an item.
		if r.Kind != core.KindKeyword || len(r.Actions) != 1 {
			t.Errorf("%+v", r)
		}
	}
	for _, r := range Search(s, "?", core.CategoryApps) {
		if r.Kind == core.KindKeyword {
			t.Errorf("a keyword row in Apps: %+v", r)
		}
	}
}

func TestAnEqualsSignListsOnlyTheCalculationHistory(t *testing.T) {
	s := empty()
	s.Calculations = calc.NewHistory([]calc.Calculation{{ID: 1, Expression: "12 * 8", Result: "96"}})
	if got := titles(Search(s, "=", core.CategoryAll)); !slices.Equal(got, []string{"96"}) {
		t.Errorf("= %q", got)
	}
	if got := Search(s, "=", core.CategoryApps); len(got) != 0 {
		t.Errorf("= in Apps: %q", titles(got))
	}
	if got := Search(s, "=5+3", core.CategoryAll)[0].Title; got != "8" {
		t.Errorf("=5+3 %q", got)
	}
}

func TestResolveFindsEachSourceByItsOwnID(t *testing.T) {
	s := empty()
	s.Library = library.NewIndex([]library.Item{
		{ID: 1, Kind: library.KindSnippet, Name: "Signature", Text: "Best"},
		quicklink(2, "Jira", "", "https://jira.test/{query}"),
	})
	for _, c := range []struct {
		id core.ID
		ok bool
	}{
		{"snippet:1", true}, {"link:2", true},
		// A snippet's ID never finds a quicklink, nor the other way round.
		{"link:1", false}, {"snippet:2", false},
		{"system:lock", true}, {"emoji:🚀", true}, {"app:/Gone.app", false},
		{"clip:x", false}, {"calc:1+1", false},
	} {
		if r, ok := Resolve(s, c.id); ok != c.ok || ok && r.ID != c.id {
			t.Errorf("%s: %q %v", c.id, r.ID, ok)
		}
	}
}

func TestReadyWaitsForTheFirstScanOfAppsAndFiles(t *testing.T) {
	s := empty()
	s.AppsReady, s.FilesReady = false, true
	for _, source := range core.Sources() {
		if want := source != core.SourceApp; s.Ready(source) != want {
			t.Errorf("%v ready: %v", source, !want)
		}
	}
}

func TestUsedOftenSuggestsUpToEightBelowThePins(t *testing.T) {
	s := empty()
	used := map[core.ID]core.Use{"system:lock": {Count: 20, LastUsed: s.Now.Unix()}}
	for _, glyph := range []string{"🚀", "🎉", "😀", "👍", "🔥", "❤️", "✅", "⭐", "🍕", "🌙"} {
		used[core.ID("emoji:"+glyph)] = core.Use{Count: 1, LastUsed: s.Now.Unix()}
	}
	s.Usage = NewUsage(used)
	s.Pins = NewPins([]core.ID{"emoji:🚀"})
	got := Search(s, "", core.CategoryAll)
	// System commands are never suggested, and the pin shows once.
	if len(got) != 1+suggestions || got[0].ID != "emoji:🚀" || slices.Contains(ids(got[1:]), "emoji:🚀") || slices.Contains(ids(got), "system:lock") {
		t.Errorf("%q", ids(got))
	}
}
