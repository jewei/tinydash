package library

import (
	"encoding/json"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/golden"
)

// engines are the web package's, as the search package passes them.
var engines = []Engine{
	{"Google", []string{"g", "google"}}, {"DuckDuckGo", []string{"ddg", "duckduckgo"}},
	{"Bing", []string{"bing"}}, {"Brave", []string{"brave"}}, {"YouTube", []string{"yt", "youtube"}},
	{"GitHub", []string{"gh", "github"}}, {"Wikipedia", []string{"w", "wiki", "wikipedia"}},
}

func keywordItem(id int64, kind Kind, name, keyword, text string) Item {
	return Item{ID: id, Kind: kind, Name: name, Keyword: keyword, Text: text}
}

func typed(r core.Result) string { return r.Actions[0].Action.(core.TypeQuery).Text }

func keywordTitles(t *testing.T, x *Index, query string) []string {
	t.Helper()
	rows, ok := x.Keywords(query, engines)
	if !ok {
		t.Fatalf("%q is not the list", query)
	}
	var out []string
	for _, r := range rows {
		out = append(out, r.Title)
	}
	return out
}

func TestAQuestionMarkListsBuiltInWebAndOwnKeywords(t *testing.T) {
	x := NewIndex([]Item{
		keywordItem(1, KindQuicklink, "Jira", "jira", "https://jira.test/{query}"),
		keywordItem(2, KindSnippet, "Address", "addr", "1 Main St"),
		keywordItem(3, KindSnippet, "No keyword", "", "text"),
		keywordItem(4, KindSnippet, "Bank PIN", "pin", "1234"),
	})
	rows, _ := x.Keywords("?", engines)
	titles := keywordTitles(t, x, "?")
	if titles[0] != "=" || !slices.Contains(titles, "g, google") {
		t.Errorf("%q", titles)
	}
	// The user's own keywords come last, by keyword.
	if last := titles[len(titles)-3:]; !slices.Equal(last, []string{"addr", "jira", "pin"}) {
		t.Errorf("last %q", last)
	}
	jira := rows[len(rows)-2]
	if jira.Subtitle != "Jira with the text after it" || typed(jira) != "jira " {
		t.Errorf("%+v", jira)
	}
	// A keyword that only finds its item is typed without a space.
	if got := typed(rows[len(rows)-3]); got != "addr" {
		t.Errorf("%q", got)
	}
	// Every ID is different, also for `pin`, built in and the user's own,
	// so the list can keep its selection.
	ids := map[core.ID]bool{}
	for _, r := range rows {
		ids[r.ID] = true
	}
	if len(ids) != len(rows) {
		t.Errorf("%d IDs for %d rows", len(ids), len(rows))
	}
}

func TestWordsAfterTheQuestionMarkFilterTheList(t *testing.T) {
	x := NewIndex([]Item{keywordItem(1, KindQuicklink, "Jira", "jira", "https://jira.test/{query}")})
	if got := keywordTitles(t, x, "?  JIRA "); len(got) != 1 {
		t.Errorf("%q", got)
	}
	rows, _ := x.Keywords("? search git", engines)
	if rows[0].Title != "gh, github" || typed(rows[0]) != "gh " {
		t.Errorf("%+v", rows[0])
	}
	// A word matches the start of a word: "digits" in the PIN row has
	// "git" inside it, not at its start.
	if got := keywordTitles(t, x, "? git"); !slices.Equal(got, []string{"gh, github"}) {
		t.Errorf("%q", got)
	}
	if got := keywordTitles(t, x, "? ="); got[0] != "=" {
		t.Errorf("%q", got)
	}
	if got := keywordTitles(t, x, "? nothing-like-this"); len(got) != 0 {
		t.Errorf("%q", got)
	}
}

func TestShellCommandsAndChmodAreNotListed(t *testing.T) {
	x := NewIndex(nil)
	for _, title := range keywordTitles(t, x, "?") {
		if title == ">" || title == "chmod" {
			t.Errorf("lists %q, a dropped feature", title)
		}
	}
}

func TestKeywordWordsMatchInEitherUnicodeForm(t *testing.T) {
	x := NewIndex([]Item{keywordItem(1, KindSnippet, "Café menu", "menu", "Soup")})
	// Pasted text can hold decomposed accents.
	if got := keywordTitles(t, x, "? café"); !slices.Equal(got, []string{"menu"}) {
		t.Errorf("%q", got)
	}
}

func TestEachBuiltInRowTypesTheStartOfItsExample(t *testing.T) {
	for _, b := range builtIns {
		if !strings.HasPrefix(b.example, strings.TrimRight(b.typed, " ")) || len([]rune(b.typed)) > MaxTyped {
			t.Errorf("%+v", b)
		}
	}
	if len(Examples()) != len(builtIns) {
		t.Error("Examples misses a row")
	}
}

func TestOtherQueriesAreNotTheList(t *testing.T) {
	x := NewIndex(nil)
	for _, query := range []string{"", "?x", "what?"} {
		if _, ok := x.Keywords(query, engines); ok {
			t.Errorf("%q is the list", query)
		}
	}
	if rows, ok := x.Keywords("? nothing", engines); !ok || rows == nil {
		t.Error("an empty list must be an answer, not nil")
	}
}

// The `?` rows of testdata/golden/search.jsonl, in its fixed world. The
// chmod row is gone with the chmod feature.
func TestKeywordRowsMatchRust(t *testing.T) {
	x := NewIndex([]Item{
		{ID: 1, Kind: KindSnippet, Name: "Signature", Keyword: "sig", Text: "Best,\nJ"},
		{ID: 2, Kind: KindSnippet, Name: "Address", Keyword: "addr", Text: "1 Main St"},
		{ID: 3, Kind: KindQuicklink, Name: "Search MDN", Keyword: "mdn", Text: "https://developer.mozilla.org/search?q={query}"},
		{ID: 4, Kind: KindQuicklink, Name: "Tinydash repo", Text: "https://github.com/jewei/tinydash"},
	})
	type row struct {
		Category string            `json:"category"`
		Query    string            `json:"query"`
		Results  []json.RawMessage `json:"results"`
	}
	for _, r := range golden.Rows[row](t, "search") {
		if r.Category != "all" || r.Query != "?" {
			continue
		}
		got, _ := x.Keywords("?", engines)
		var want []json.RawMessage
		for _, raw := range r.Results {
			var head struct{ ID string }
			if err := json.Unmarshal(raw, &head); err != nil {
				t.Fatal(err)
			}
			if head.ID != "keyword:chmod" {
				want = append(want, raw)
			}
		}
		if len(got) != len(want) {
			t.Fatalf("%d rows, Rust had %d without chmod", len(got), len(want))
		}
		for i := range want {
			golden.Check(t, "keywords", string(got[i].ID), want[i], golden.Result(got[i]))
		}
		return
	}
	t.Fatal("no `?` search in the corpus")
}
