package library

import (
	"cmp"
	"errors"
	"fmt"
	"slices"
	"strings"
	"time"
	"unicode"

	"golang.org/x/text/unicode/norm"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

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
}

func (c Context) boost(id core.ID) uint32 {
	if c.Personal == nil {
		return 0
	}
	return c.Personal.Boost(id)
}

func (c Context) pinned(id core.ID) bool { return c.Personal != nil && c.Personal.Pinned(id) }

// Index is the saved items: snippets first, then quicklinks, each by name.
type Index struct {
	items []Item
}

// NewIndex puts names and keywords into NFC, also for items saved before
// Validated did that, and sorts the items.
func NewIndex(items []Item) *Index {
	sorted := make([]Item, len(items))
	for i, it := range items {
		it.Name = norm.NFC.String(it.Name)
		it.Keyword = norm.NFC.String(it.Keyword)
		sorted[i] = it
	}
	slices.SortStableFunc(sorted, func(a, b Item) int {
		return cmp.Or(cmp.Compare(rank(a.Kind), rank(b.Kind)), cmp.Compare(strings.ToLower(a.Name), strings.ToLower(b.Name)))
	})
	return &Index{sorted}
}

func rank(k Kind) int {
	if k == KindQuicklink {
		return 1
	}
	return 0
}

// Items lists the items in index order.
func (x *Index) Items() []Item { return x.items }

// Find is the saved item with this ID.
func (x *Index) Find(id int64) (Item, bool) {
	i := slices.IndexFunc(x.items, func(it Item) bool { return it.ID == id })
	if i < 0 {
		return Item{}, false
	}
	return x.items[i], true
}

// CheckKind refuses to change a saved item between snippet and quicklink:
// its result ID would change, and its pins, aliases, hidden rows, and hotkey
// would stay with the old one.
func (x *Index) CheckKind(it Item) error {
	if saved, ok := x.Find(it.ID); ok && saved.Kind != it.Kind {
		return errors.New("A snippet cannot become a quicklink, or a quicklink a snippet. Add a new item instead.")
	}
	return nil
}

// CheckKeyword refuses a keyword that another item already has: only one
// of them could answer it.
func (x *Index) CheckKeyword(it Item) error {
	if it.Keyword == "" {
		return nil
	}
	for _, other := range x.items {
		if other.ID != it.ID && other.Keyword == it.Keyword {
			return fmt.Errorf("The keyword “%s” is used by %s. Choose another keyword.", it.Keyword, other.Name)
		}
	}
	return nil
}

// Search ranks the items whose name or keyword matches; the best limit
// come first.
func (x *Index) Search(m *match.Matcher, ctx Context, limit int) []core.Scored {
	var hits []match.Hit[*Item]
	for i := range x.items {
		it := &x.items[i]
		var keyword []string
		if it.Keyword != "" {
			keyword = []string{it.Keyword}
		}
		if score, ok := m.Best(it.Name, keyword...); ok {
			hits = append(hits, match.Hit[*Item]{Score: score + ctx.boost(it.resultID()), Item: it})
		}
	}
	top := match.Top(hits, limit)
	out := make([]core.Scored, len(top))
	for i, hit := range top {
		out[i] = core.Scored{Score: hit.Score, Result: result(hit.Item, "", ctx)}
	}
	return out
}

// Browse lists every item.
func (x *Index) Browse(ctx Context) []core.Result {
	out := make([]core.Result, len(x.items))
	for i := range x.items {
		out[i] = result(&x.items[i], "", ctx)
	}
	return out
}

// Get is the result of the item with this ID.
func (x *Index) Get(id int64, ctx Context) (core.Result, bool) {
	it, ok := x.Find(id)
	if !ok {
		return core.Result{}, false
	}
	return result(&it, "", ctx), true
}

// Fallbacks are the quicklinks with {query} that can open the whole query,
// most used first, for the end of an All search.
func (x *Index) Fallbacks(query string, ctx Context, now time.Time, home string) []core.Result {
	var links []*Item
	for i := range x.items {
		it := &x.items[i]
		if it.Kind != KindQuicklink || !strings.Contains(it.Text, "{query}") {
			continue
		}
		if _, err := QuicklinkTarget(it.Text, query, now, home, nil); err == nil {
			links = append(links, it)
		}
	}
	// Stable, so equally used links keep their name order.
	slices.SortStableFunc(links, func(a, b *Item) int { return cmp.Compare(ctx.boost(b.resultID()), ctx.boost(a.resultID())) })
	out := make([]core.Result, len(links))
	for i, it := range links {
		out[i] = answer(it, query)
	}
	return out
}

// KeywordAnswer runs `keyword text` for a quicklink, or a snippet with
// {query}, whose keyword is the first word.
func (x *Index) KeywordAnswer(query string) (core.Result, bool) {
	at := strings.IndexFunc(query, unicode.IsSpace)
	if at < 0 {
		return core.Result{}, false
	}
	keyword, text := match.Normalize(query[:at]), strings.TrimSpace(query[at:])
	for i := range x.items {
		if it := &x.items[i]; it.TakesQuery() && it.Keyword == keyword {
			return answer(it, text), true
		}
	}
	return core.Result{}, false
}

// answer runs an item with the typed text. It has no Pin: a pin keeps the
// item, not the text.
func answer(it *Item, query string) core.Result {
	r := result(it, query, Context{})
	r.Actions = r.Actions[:1]
	return r
}

func result(it *Item, query string, ctx Context) core.Result {
	id := it.resultID()
	var r core.Result
	switch it.Kind {
	case KindQuicklink:
		subtitle := it.Text
		if query != "" {
			subtitle = strings.ReplaceAll(it.Text, "{query}", query)
		}
		r = core.Result{
			Kind: core.KindQuicklink, Subtitle: subtitle, Icon: core.SymbolIcon(core.SymbolLink),
			Actions: []core.ResultAction{core.Do("Open Quicklink", core.OpenQuicklink{ID: it.ID, Query: query})},
		}
	default:
		line, _, _ := strings.Cut(it.Text, "\n")
		line = strings.TrimSuffix(line, "\r")
		if query != "" {
			line = strings.ReplaceAll(line, "{query}", query)
		}
		r = core.Result{
			Kind: core.KindSnippet, Subtitle: firstChars(line, 80), Icon: core.SymbolIcon(core.SymbolSnippet),
			Actions: []core.ResultAction{core.Do("Copy Snippet", core.CopySnippet{ID: it.ID, Query: query})},
		}
	}
	r.ID = id
	r.Title = it.Name
	if query != "" {
		r.Title = it.Name + ": " + query
	}
	r.Pinned = ctx.pinned(id)
	if r.Pinned {
		r.Actions = append(r.Actions, core.Do("Unpin", core.Unpin{ID: id}))
	} else {
		r.Actions = append(r.Actions, core.Do("Pin", core.Pin{ID: id}))
	}
	return r
}

func firstChars(s string, n int) string {
	for i := range s {
		if n == 0 {
			return s[:i]
		}
		n--
	}
	return s
}
