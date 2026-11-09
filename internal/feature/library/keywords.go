package library

import (
	"slices"
	"strconv"
	"strings"
	"unicode"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// keywordRows is the most rows in the `?` list. Up to 500 snippets and
// quicklinks can have keywords, so `? words` finds the ones past it.
const keywordRows = 100

// MaxTyped is the longest text a `?` row types: a keyword of up to 32
// characters and a space, with room to spare. The engine refuses longer
// text.
const MaxTyped = 64

// builtIn is a built-in way to start a query. The search package tests
// that each example gets an answer, so this list cannot drift from the
// features.
type builtIn struct {
	keywords, what, example string
	// typed is what Enter types: the keyword, with a space when text
	// follows it.
	typed  string
	symbol core.Symbol
}

var builtIns = []builtIn{
	{"=", "Calculations you copied", "= cm", "=", core.SymbolCalculator},
	{"now, time in", "The time here or in a city", "time in tokyo", "time in ", core.SymbolClock},
	{"today, in", "Count days from a date", "today + 3 days", "today + ", core.SymbolClock},
	{"days until, days since, days between", "Count the days to or from a date", "days until dec 25", "days until ", core.SymbolClock},
	{"password, pw", "A password, a passphrase, and a PIN", "pw 32", "pw ", core.SymbolKey},
	{"passphrase", "A passphrase of random words", "passphrase 8", "passphrase ", core.SymbolKey},
	{"pin", "A PIN of random digits", "pin 4", "pin ", core.SymbolKey},
}

// Examples are the examples of the built-in rows, for the search package's
// test that each one gets an answer.
func Examples() []string {
	out := make([]string, len(builtIns))
	for i, b := range builtIns {
		out[i] = b.example
	}
	return out
}

// Engine is a web search engine and its keywords, shortest first, as the
// web package lists them.
type Engine struct {
	Name     string
	Keywords []string
}

// Keywords answers `?` or `? words` in All: the words that start something,
// built in, web engines, then the user's own keywords. `? words` keeps the
// rows that hold every word. ok is false for any other query.
func (x *Index) Keywords(query string, engines []Engine) (rows []core.Result, ok bool) {
	filter, ok := strings.CutPrefix(strings.TrimSpace(query), "?")
	if !ok {
		return nil, false
	}
	// `?x` is not this list; a space must follow, as for `=`.
	if filter != "" && !unicode.IsSpace([]rune(filter)[0]) {
		return nil, false
	}
	var all []core.Result
	for _, b := range builtIns {
		all = append(all, keywordRow("keyword:"+b.keywords, b.keywords, b.what+" · "+b.example, b.typed, b.symbol))
	}
	for _, e := range engines {
		first := e.Keywords[0]
		all = append(all, keywordRow("keyword:"+e.Name, strings.Join(e.Keywords, ", "), "Search "+e.Name+" · "+first+" rust", first+" ", core.SymbolGlobe))
	}
	var own []*Item
	for i := range x.items {
		if x.items[i].Keyword != "" {
			own = append(own, &x.items[i])
		}
	}
	slices.SortStableFunc(own, func(a, b *Item) int { return strings.Compare(a.Keyword, b.Keyword) })
	for _, it := range own {
		symbol := core.SymbolSnippet
		if it.Kind == KindQuicklink {
			symbol = core.SymbolLink
		}
		what, typed := it.Name, it.Keyword
		if it.TakesQuery() {
			what, typed = it.Name+" with the text after it", it.Keyword+" "
		}
		// Its own ID: a keyword may equal a built-in one, such as `pin`.
		id := "keyword:item:" + strconv.FormatInt(it.ID, 10)
		all = append(all, keywordRow(id, it.Keyword, what, typed, symbol))
	}
	words := strings.Fields(match.Normalize(filter))
	rows = []core.Result{}
	for _, r := range all {
		if len(rows) == keywordRows {
			break
		}
		if holdsWords(r, words) {
			rows = append(rows, r)
		}
	}
	return rows, true
}

// holdsWords reports whether each word starts a word of the row, so `git`
// finds GitHub but not "digits".
func holdsWords(r core.Result, words []string) bool {
	parts := strings.FieldsFunc(match.Normalize(r.Title+" "+r.Subtitle), func(c rune) bool {
		return unicode.IsSpace(c) || c == ',' || c == '·'
	})
	for _, word := range words {
		if !slices.ContainsFunc(parts, func(part string) bool { return strings.HasPrefix(part, word) }) {
			return false
		}
	}
	return true
}

func keywordRow(id, keywords, subtitle, typed string, symbol core.Symbol) core.Result {
	return core.Result{
		ID:       core.ID(id),
		Kind:     core.KindKeyword,
		Title:    keywords,
		Subtitle: subtitle,
		Icon:     core.SymbolIcon(symbol),
		Actions:  []core.ResultAction{core.Do("Use Keyword", core.TypeQuery{Text: typed})},
	}
}
