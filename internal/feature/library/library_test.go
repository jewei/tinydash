package library

import (
	"path/filepath"
	"slices"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/match"
)

var home = golden.Abs("/Users/me")

var now = time.Date(2026, 10, 4, 9, 5, 0, 0, time.UTC)

func item(kind Kind, name, keyword, text string) Item {
	return Item{ID: 1, Kind: kind, Name: name, Keyword: keyword, Text: text}
}

func clip(text string) func() string { return func() string { return text } }

func target(template, query string) error {
	_, err := QuicklinkTarget(template, query, now, home, nil)
	return err
}

func TestValidationTrimsAndRejectsUnsafeQuicklinks(t *testing.T) {
	ok, err := item(KindQuicklink, " Jira ", " JIRA ", " https://jira.test/browse/{query} ").Validated(now, home)
	if err != nil || ok.Name != "Jira" || ok.Keyword != "jira" || ok.Text != "https://jira.test/browse/{query}" {
		t.Fatalf("%+v %v", ok, err)
	}
	for _, bad := range []Item{
		item(KindSnippet, "", "", "x"),
		item(KindSnippet, "a", "two words", "x"),
		item(KindSnippet, "a", "", " \n"),
		item(KindQuicklink, "a", "", "file:///etc/passwd"),
		item(KindQuicklink, "a", "", "javascript:alert(1)"),
		item(KindQuicklink, "a", "", "ms-settings:"),
		item(KindQuicklink, "a", "", "relative/path"),
		item(KindQuicklink, "a", "", "{query}/x"),
		item(KindQuicklink, "a", "", "javascript:{query}"),
		item(KindQuicklink, "a", "", "ms-settings:{query}"),
	} {
		if _, err := bad.Validated(now, home); err == nil {
			t.Errorf("%+v passed", bad)
		}
	}
	// A template that works once text is typed after its keyword can be
	// saved, even if it fails with nothing typed.
	for _, good := range []string{"~/Projects", "https://{query}", "~/a/b/{query}/../c", "http://localhost:{query}"} {
		if _, err := item(KindQuicklink, "a", "", good).Validated(now, home); err != nil {
			t.Errorf("%s: %v", good, err)
		}
	}
}

func TestValidationMessages(t *testing.T) {
	for _, c := range []struct {
		item Item
		want string
	}{
		{item(KindSnippet, "", "", "x"), "Enter a name of up to 100 characters."},
		{item(KindSnippet, "a", "a b", "x"), "A keyword is one word of up to 32 characters."},
		{item(KindSnippet, "a", "", ""), "Enter the text, up to 32 KB."},
		{item(KindQuicklink, "a", "", "https://"), "Invalid quicklink URL: empty host"},
		{item(KindQuicklink, "a", "", "relative"), "A quicklink opens an http, https, or mailto URL, or an absolute or ~ path."},
	} {
		if _, err := c.item.Validated(now, home); err == nil || err.Error() != c.want {
			t.Errorf("%+v: %v, want %q", c.item, err, c.want)
		}
	}
}

func TestPathQuicklinksTakeANameNotAPath(t *testing.T) {
	for _, c := range []struct {
		template, query string
		ok              bool
	}{
		{"~/Projects/{query}", "../../Downloads/x.app", false},
		{"~/Projects/{query}", "notes", true},
		{"~/Projects/{query}{query}", ".", false},
		{"~/Projects/{query}", ".notes", true},
		{"~/Projects/../Downloads/{query}", "x", true},
		// Nor can a value pick the root, such as the home folder or a
		// network server, or stay put with dots Windows drops.
		{"{query}/x", "~", false},
		{`\\{query}\share`, "attacker", false},
		{"~/Notes/{query}", "...", false},
		{"~/Notes/{query}", ". ", false},
		{"~/Notes/{query}", "", true},
		{"~/x/{query}/y", "", true},
		// An empty value before the template's own `..` would climb higher.
		{"~/a/b/{query}/../c", "", false},
		{"~/a/b/{query}/../c", "n", true},
	} {
		if err := target(c.template, c.query); (err == nil) != c.ok {
			t.Errorf("%s with %q: %v", c.template, c.query, err)
		}
	}
	for _, c := range []struct {
		template, clipboard string
	}{
		{"~/Projects/{clipboard}", "../secret"},
		{"~/Projects/.{clipboard}", "."},
		// Values cannot cancel the template's own `..` to climb higher.
		{"~/root/{clipboard}{clipboard}/x/{query}..", "."},
		{"~/a/b/{clipboard}/../../c", ""},
	} {
		if _, err := QuicklinkTarget(c.template, "q", now, home, clip(c.clipboard)); err == nil {
			t.Errorf("%s with clipboard %q passed", c.template, c.clipboard)
		}
	}
	got, err := QuicklinkTarget("~/Projects/{query}", "notes", now, home, nil)
	// Windows takes both separators; the Rust app joined them the same way.
	if err != nil || got.URL != "" || filepath.Clean(got.Path) != golden.Abs("/Users/me/Projects/notes") {
		t.Errorf("%+v %v", got, err)
	}
	if got, _ := QuicklinkTarget("~", "", now, home, nil); got.Path != home {
		t.Errorf("~ is %q", got.Path)
	}
}

func TestURLChecks(t *testing.T) {
	for url, ok := range map[string]bool{
		"https://x.test/a b":         true,
		"https://":                   false,
		"https:///x":                 true,
		"https://user@":              false,
		"http://localhost:":          true,
		"http://localhost:8080/x":    true,
		"http://localhost:x":         false,
		"http://localhost:65536":     false,
		"https://a%20b.test":         false,
		"https://a b.test":           false,
		"https://[::1]:80/":          true,
		"https://[::1":               false,
		"https://1":                  true,
		"https://1.2.3.4":            true,
		"https://1.2.3.999":          false,
		"https://0x7f.1":             true,
		"https://example.com.":       true,
		"https://東京.test":            true,
		"mailto:":                    true,
		"mailto:me@x.test?subject=a": true,
		"HTTPS://X.TEST":             true,
	} {
		if err := checkURL(url); (err == nil) != ok {
			t.Errorf("%s: %v", url, err)
		}
	}
}

func TestFillsQuicklinksAndSnippets(t *testing.T) {
	got, err := QuicklinkTarget("https://x.test/?q={query}&d={date}", "a b", now, home, nil)
	if err != nil || got.URL != "https://x.test/?q=a%20b&d=2026-10-04" {
		t.Errorf("%+v %v", got, err)
	}
	got, err = QuicklinkTarget("https://x.test/{clipboard}", "", now, home, clip("a/b"))
	if err != nil || got.URL != "https://x.test/a%2Fb" {
		t.Errorf("%+v %v", got, err)
	}
	if text := RenderSnippet("{date} {time} {clipboard} {query} {datetime}", "Sam", now, clip("clip")); text != "2026-10-04 09:05 clip Sam 2026-10-04 09:05" {
		t.Errorf("%q", text)
	}
	read := false
	RenderSnippet("no placeholders", "", now, func() string { read = true; return "" })
	if read {
		t.Error("read the clipboard without {clipboard}")
	}
}

func TestValuesNeverFillInTurn(t *testing.T) {
	reads := 0
	text := RenderSnippet("{query} {clipboard}{clipboard} {x} {", "{clipboard}", now, func() string {
		reads++
		return "{date}"
	})
	if text != "{clipboard} {date}{date} {x} {" || reads != 1 {
		t.Errorf("%q, %d reads", text, reads)
	}
	if text := RenderSnippet("{{query}}", "a", now, nil); text != "{a}" {
		t.Errorf("%q", text)
	}
}

func TestAKeywordBelongsToOneItem(t *testing.T) {
	x := NewIndex([]Item{item(KindQuicklink, "Jira", "j", "https://jira.test/browse/{query}")})
	jest := item(KindSnippet, "Jest template", "j", "test({query})")
	jest.ID = 0
	err := x.CheckKeyword(jest)
	if err == nil || err.Error() != "The keyword “j” is used by Jira. Choose another keyword." {
		t.Errorf("%v", err)
	}
	saved := jest
	saved.ID = 2
	if x.CheckKeyword(saved) == nil {
		t.Error("a saved item took a used keyword")
	}
	// Jira keeps its own keyword when it is saved again.
	if err := x.CheckKeyword(x.Items()[0]); err != nil {
		t.Error(err)
	}
	for _, keyword := range []string{"jt", ""} {
		free := jest
		free.Keyword = keyword
		if err := x.CheckKeyword(free); err != nil {
			t.Errorf("%q: %v", keyword, err)
		}
	}
}

func TestASavedItemKeepsItsKind(t *testing.T) {
	x := NewIndex([]Item{item(KindSnippet, "Sig", "", "Regards")})
	saved := x.Items()[0]
	if err := x.CheckKind(saved); err != nil {
		t.Error(err)
	}
	changed := saved
	changed.Kind, changed.Text = KindQuicklink, "https://x.test"
	if x.CheckKind(changed) == nil {
		t.Error("a snippet became a quicklink")
	}
	// A new item, or one that was deleted meanwhile, has no kind to keep.
	for _, id := range []int64{0, 9} {
		other := changed
		other.ID = id
		if err := x.CheckKind(other); err != nil {
			t.Errorf("%d: %v", id, err)
		}
	}
}

func TestNamesAndKeywordsMatchInEitherUnicodeForm(t *testing.T) {
	// A name pasted from a macOS file name can hold decomposed accents.
	decomposed := item(KindQuicklink, "Résumé", "café", "https://x.test/{query}")
	x := NewIndex([]Item{decomposed})
	if found := x.Search(match.New("résumé"), Context{}, 10); len(found) != 1 {
		t.Errorf("%d found", len(found))
	}
	for _, query := range []string{"café x", "café x"} {
		if _, ok := x.KeywordAnswer(query); !ok {
			t.Errorf("%q has no answer", query)
		}
	}
	saved, err := decomposed.Validated(now, home)
	if err != nil || saved.Name != "Résumé" || saved.Keyword != "café" {
		t.Errorf("%+v %v", saved, err)
	}
	// The other form of a keyword is the same keyword.
	other := item(KindSnippet, "Other", "café", "x")
	other.ID = 2
	other, _ = other.Validated(now, home)
	if x.CheckKeyword(other) == nil {
		t.Error("the other form is a new keyword")
	}
}

func TestKeywordsTakeTheRestOfTheQuery(t *testing.T) {
	greeting := item(KindSnippet, "Greeting", "hi", "Hello {query},")
	greeting.ID = 2
	signature := item(KindSnippet, "Sig", "sig", "Regards")
	signature.ID = 3
	x := NewIndex([]Item{item(KindQuicklink, "Jira", "jira", "https://jira.test/browse/{query}"), greeting, signature})
	a, ok := x.KeywordAnswer("jira ABC-12")
	if !ok || a.Title != "Jira: ABC-12" || a.Actions[0].Action != (core.OpenQuicklink{ID: 1, Query: "ABC-12"}) {
		t.Errorf("%+v", a)
	}
	for _, query := range []string{"jira", "jiraa x", "sig Sam"} {
		if _, ok := x.KeywordAnswer(query); ok {
			t.Errorf("%q has an answer", query)
		}
	}
	hello, _ := x.KeywordAnswer("HI Sam")
	if hello.Title != "Greeting: Sam" || hello.Subtitle != "Hello Sam," || len(hello.Actions) != 1 || hello.Actions[0].Action != (core.CopySnippet{ID: 2, Query: "Sam"}) {
		t.Errorf("%+v", hello)
	}
	// Its own row shows where the text goes.
	if own, _ := x.Get(2, Context{}); own.Subtitle != "Hello {query}," {
		t.Errorf("%q", own.Subtitle)
	}
}

func TestFallbacksAreQuicklinksThatCanOpenTheQuery(t *testing.T) {
	link := func(id int64, name, text string) Item {
		it := item(KindQuicklink, name, "", text)
		it.ID = id
		return it
	}
	x := NewIndex([]Item{
		link(1, "Jira", "https://jira.test/browse/{query}"),
		link(2, "Docs", "https://docs.test/"),
		link(3, "Notes", "~/Notes/{query}"),
		item(KindSnippet, "Hi", "", "Hello {query}"),
	})
	titles := func(query string) []string {
		var out []string
		for _, r := range x.Fallbacks(query, Context{}, now, home) {
			out = append(out, r.Title)
		}
		return out
	}
	if got := titles("ABC-12"); !slices.Equal(got, []string{"Jira: ABC-12", "Notes: ABC-12"}) {
		t.Errorf("%q", got)
	}
	// A path quicklink takes a name, not a path.
	if got := titles("a/b"); !slices.Equal(got, []string{"Jira: a/b"}) {
		t.Errorf("%q", got)
	}
	if jira := x.Fallbacks("x", Context{}, now, home)[0]; len(jira.Actions) != 1 {
		t.Errorf("a fallback has %d actions", len(jira.Actions))
	}
}

type personal struct {
	boost map[core.ID]uint32
	pins  map[core.ID]bool
}

func (p personal) Boost(id core.ID) uint32 { return p.boost[id] }
func (p personal) Pinned(id core.ID) bool  { return p.pins[id] }

// The fixed world of testdata/golden/search.jsonl, and the rows Rust gave.
func TestResultsHaveTheRustShape(t *testing.T) {
	x := NewIndex([]Item{
		{ID: 1, Kind: KindSnippet, Name: "Signature", Keyword: "sig", Text: "Best,\nJ"},
		{ID: 2, Kind: KindSnippet, Name: "Address", Keyword: "addr", Text: "1 Main St"},
		{ID: 3, Kind: KindQuicklink, Name: "Search MDN", Keyword: "mdn", Text: "https://developer.mozilla.org/search?q={query}"},
		{ID: 4, Kind: KindQuicklink, Name: "Tinydash repo", Text: "https://github.com/jewei/tinydash"},
	})
	var ids []core.ID
	for _, r := range x.Browse(Context{}) {
		ids = append(ids, r.ID)
	}
	if want := []core.ID{"snippet:2", "snippet:1", "link:3", "link:4"}; !slices.Equal(ids, want) {
		t.Errorf("order %q", ids)
	}
	sig, _ := x.Get(1, Context{Personal: personal{pins: map[core.ID]bool{"snippet:1": true}}})
	if sig.Subtitle != "Best," || sig.Kind != core.KindSnippet || sig.Icon.Symbol != core.SymbolSnippet || !sig.Pinned ||
		sig.Actions[0] != core.Do("Copy Snippet", core.CopySnippet{ID: 1}) || sig.Actions[1] != core.Do("Unpin", core.Unpin{ID: "snippet:1"}) {
		t.Errorf("%+v", sig)
	}
	mdn := x.Fallbacks("array map", Context{}, now, home)
	if len(mdn) != 1 || mdn[0].ID != "link:3" || mdn[0].Title != "Search MDN: array map" ||
		mdn[0].Subtitle != "https://developer.mozilla.org/search?q=array map" || mdn[0].Icon.Symbol != core.SymbolLink {
		t.Errorf("%+v", mdn)
	}
	if hits := x.Search(match.New("sig"), Context{}, 10); len(hits) == 0 || hits[0].Result.ID != "snippet:1" {
		t.Errorf("%+v", hits)
	}
}
