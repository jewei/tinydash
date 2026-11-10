// Package emoji finds emoji by English name, GitHub shortcode, and CLDR
// keywords in the languages the user chose.
package emoji

import (
	"cmp"
	"embed"
	"slices"
	"strings"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// Language is an extra keyword language. English names and shortcodes are
// always on.
type Language string

const (
	Chinese Language = "zh" // Simplified
	Malay   Language = "ms"
	Spanish Language = "es"
)

// The CLDR tables that scripts/emoji-data.ts writes: per row, the emoji, its
// short name, then related keywords, tab-separated.
//
//go:embed data/*.tsv
var tables embed.FS

// emoji is one row of the catalog (catalog.go).
type emoji struct {
	glyph      string
	name       string
	shortcodes []string
	tones      tones
}

// tones places an emoji among its skin-tone variants: the catalog index of
// the default-tone one, how many there are, and its offset from that one.
// A zero count means skin tones do not apply.
type tones struct {
	base, count, offset int
}

// SkinTone is the tone results show: 0 is the default (yellow), 1 to 5 go
// from light to dark.
type SkinTone uint8

// Context is how one search ranks and shows its results.
type Context struct {
	Personal core.Personal
	SkinTone SkinTone
}

func (c Context) boost(id core.ID) uint32 {
	if c.Personal == nil {
		return 0
	}
	return c.Personal.Boost(id)
}

func (c Context) pinned(id core.ID) bool { return c.Personal != nil && c.Personal.Pinned(id) }

type entry struct {
	emoji    int // catalog index
	names    []string
	keywords []string
}

// Index is the searchable catalog: default-tone emoji only, in catalog order.
type Index struct {
	entries []entry
}

// NewIndex builds the index with keywords in the given languages; a
// language without a table adds nothing.
func NewIndex(languages ...Language) *Index {
	var entries []entry
	position := map[int]int{}
	for i, e := range catalog {
		if e.tones.count == 0 || e.tones.offset == 0 {
			position[i] = len(entries)
			entries = append(entries, entry{emoji: i, names: slices.Clone(e.shortcodes)})
		}
	}
	for _, language := range languages {
		table, err := tables.ReadFile("data/" + string(language) + ".tsv")
		if err != nil {
			continue
		}
		for row := range strings.Lines(string(table)) {
			fields := strings.Split(strings.TrimSuffix(row, "\n"), "\t")
			found, ok := lookup(fields[0])
			if !ok {
				continue
			}
			i, ok := position[found]
			if !ok {
				continue
			}
			e := &entries[i]
			if len(fields) > 1 && fields[1] != "" {
				e.names = append(e.names, fields[1])
			}
			if len(fields) > 2 {
				e.keywords = append(e.keywords, fields[2:]...)
			}
		}
	}
	return &Index{entries}
}

// lookup finds an emoji by its glyph, in any form emojis::get accepts.
func lookup(glyph string) (int, bool) {
	if i, ok := glyphs[glyph]; ok {
		return i, true
	}
	i, ok := variants[glyph]
	return i, ok
}

var glyphs = func() map[string]int {
	m := make(map[string]int, len(catalog))
	for i, e := range catalog {
		m[e.glyph] = i
	}
	return m
}()

// Search ranks the emoji that match the query; the best limit come first.
func (x *Index) Search(m *match.Matcher, ctx Context, limit int) []core.Scored {
	// Results show shortcodes as `:rocket:`; typing that form, or just the
	// opening colon, matches like `rocket`.
	if code, ok := strings.CutPrefix(m.Query(), ":"); ok {
		code = strings.TrimSuffix(code, ":")
		if code != "" {
			m = match.New(code)
		}
	}
	var hits []match.Hit[int]
	for _, e := range x.entries {
		score, ok := m.Best(catalog[e.emoji].name, e.names...)
		keyword, found := uint32(0), false
		for _, k := range e.keywords {
			if s, ok := m.Name(k); ok && (!found || s > keyword) {
				keyword, found = s, true
			}
		}
		if found {
			keyword = max(keyword, match.KeywordPenalty) - match.KeywordPenalty
			if !ok || keyword > score {
				score, ok = keyword, true
			}
		}
		if ok {
			hits = append(hits, match.Hit[int]{Score: score + ctx.boost(id(e.emoji)), Item: e.emoji})
		}
	}
	top := match.Top(hits, limit)
	out := make([]core.Scored, len(top))
	for i, hit := range top {
		out[i] = core.Scored{Score: hit.Score, Result: result(hit.Item, ctx)}
	}
	return out
}

// Browse lists the most used emoji first, then the catalog.
func (x *Index) Browse(ctx Context, limit int) []core.Result {
	var used, rest []int
	for _, e := range x.entries {
		if ctx.boost(id(e.emoji)) > 0 {
			used = append(used, e.emoji)
		} else {
			rest = append(rest, e.emoji)
		}
	}
	slices.SortStableFunc(used, func(a, b int) int { return cmp.Compare(ctx.boost(id(b)), ctx.boost(id(a))) })
	var out []core.Result
	for _, e := range slices.Concat(used, rest) {
		if len(out) == limit {
			break
		}
		out = append(out, result(e, ctx))
	}
	return out
}

// Get is the result of an emoji in any of its forms or skin tones.
func (x *Index) Get(glyph string, ctx Context) (core.Result, bool) {
	i, ok := lookup(glyph)
	if !ok {
		return core.Result{}, false
	}
	return result(base(i), ctx), true
}

// base is the default-tone emoji: pins and usage use it, so they survive a
// change of skin tone.
func base(i int) int {
	if catalog[i].tones.count == 0 {
		return i
	}
	return catalog[i].tones.base
}

func toned(i int, tone SkinTone) int {
	t := catalog[i].tones
	if tone < 1 || tone > 5 || int(tone) >= t.count {
		return i
	}
	return t.base + int(tone)
}

func id(i int) core.ID { return core.SourceEmoji.ID(catalog[i].glyph) }

func result(i int, ctx Context) core.Result {
	resultID := id(i)
	shown := catalog[toned(i, ctx.SkinTone)]
	title := shown.name
	if title != "" && title[0] < 0x80 {
		title = strings.ToUpper(title[:1]) + title[1:]
	}
	var subtitle string
	if codes := catalog[i].shortcodes; len(codes) > 0 {
		subtitle = ":" + codes[0] + ":"
	}
	pin := core.Do("Pin", core.Pin{ID: resultID})
	pinned := ctx.pinned(resultID)
	if pinned {
		pin = core.Do("Unpin", core.Unpin{ID: resultID})
	}
	return core.Result{
		ID:       resultID,
		Kind:     core.KindEmoji,
		Title:    title,
		Subtitle: subtitle,
		Icon:     core.EmojiIcon(shown.glyph),
		Actions:  []core.ResultAction{core.Do("Copy Emoji", core.Copy{Text: shown.glyph}), pin},
		Pinned:   pinned,
	}
}
