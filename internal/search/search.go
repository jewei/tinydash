// Package search turns a query into ranked results. It reads a Snapshot of
// the indexes and the user's personal lists, and never touches the OS, the
// database, or the clock.
package search

import (
	"io"
	"math"
	"slices"
	"strconv"
	"strings"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/calc"
	"github.com/jewei/tinydash/internal/feature/cleanurl"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/color"
	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/feature/datetime"
	"github.com/jewei/tinydash/internal/feature/emoji"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/feature/password"
	"github.com/jewei/tinydash/internal/feature/system"
	"github.com/jewei/tinydash/internal/feature/unixtime"
	"github.com/jewei/tinydash/internal/feature/weather"
	"github.com/jewei/tinydash/internal/feature/web"
	"github.com/jewei/tinydash/internal/match"
	"github.com/jewei/tinydash/internal/settings"
)

const (
	// AllLimit is the most results of an All search with text. Instant
	// answers and the web fallback count toward it. The empty All view
	// stops at CategoryLimit, like a category.
	AllLimit = 30
	// CategoryLimit is the most results in a single category.
	CategoryLimit = 100
	// quicklinkFallbacks is the most quicklinks offered after the web
	// search at the end of an All search.
	quicklinkFallbacks = 3
	// suggestions is the most "used often" rows below the pins in an empty
	// All search.
	suggestions = 8
	// unscored marks rows that are not ranked by score: answers, browse
	// rows, and fallbacks.
	unscored = math.MaxUint32
)

// Snapshot is everything a search reads. Every field is set, and nothing in
// it changes after the engine publishes it.
type Snapshot struct {
	Settings     settings.Settings
	Apps         *apps.Index
	Files        *files.Index
	Clipboard    *clipboard.History
	Library      *library.Index
	Calculations calc.History
	Emoji        *emoji.Index
	// Rates is nil while currency rates are off or not downloaded yet.
	Rates *currency.Rates
	// Weather is the latest weather download, of any city, or nil.
	Weather *weather.Weather
	Usage   Usage
	Pins    Pins
	Hidden  Hidden
	Aliases Aliases
	Hotkeys Hotkeys
	// Now is the clock of the search, in the user's time zone.
	Now time.Time
	// Home is the user's home folder, shown as "~".
	Home string
	// Random draws passwords.
	Random io.Reader
	// AppsReady and FilesReady: the first scan of the source finished.
	// Until then it looks empty.
	AppsReady, FilesReady bool
}

// Ready reports whether a source has its items. A source that a scan fills
// looks empty until its first scan finishes.
func (s *Snapshot) Ready(source core.Source) bool {
	switch source {
	case core.SourceApp:
		return s.AppsReady
	case core.SourceFile:
		return s.FilesReady
	}
	return true
}

// view is one search's reading of a snapshot: the contexts each source
// ranks and shows its results with.
type view struct {
	*Snapshot
	personal core.Personal
}

func newView(s *Snapshot) view {
	return view{s, personal{s.Usage, s.Pins, s.Now.Unix()}}
}

func (v view) apps() apps.Context { return apps.Context{Personal: v.personal, Home: v.Home} }
func (v view) files() files.Context {
	return files.Context{Personal: v.personal, Home: v.Home}
}

// clips is the clipboard history that search reads. While history is off,
// saved entries stay in the database but are hidden, so no result can act
// on an entry the user cannot see.
func (v view) clips() *clipboard.History {
	if !v.Settings.ClipboardHistoryEnabled {
		return noClips
	}
	return v.Clipboard
}

var noClips = clipboard.NewHistory(nil)

func (v view) clipboard() clipboard.Context {
	return clipboard.Context{Personal: v.personal, Now: v.Now.Unix()}
}
func (v view) library() library.Context { return library.Context{Personal: v.personal} }
func (v view) emoji() emoji.Context {
	return emoji.Context{Personal: v.personal, SkinTone: emoji.SkinTone(v.Settings.EmojiSkinTone)}
}
func (v view) system() system.Context { return system.Context{Personal: v.personal} }

// keywordEngines are the web engines in the `?` list.
var keywordEngines = func() []library.Engine {
	var out []library.Engine
	for _, e := range web.Engines() {
		out = append(out, library.Engine{Name: e.Name, Keywords: e.Keywords})
	}
	return out
}()

// Search returns the results for a query in a category, without the ones
// the user hid, and with a way to give each of the others an alias or a
// hotkey, or to hide it. An empty query lists the category.
func Search(s *Snapshot, query string, category core.Category) []core.Result {
	scored := search(s, query, category)
	out := make([]core.Result, len(scored))
	for i, hit := range scored {
		out[i] = hit.Result
	}
	return out
}

// search is Search with the score that placed each ranked row; other rows
// have the score unscored.
func search(s *Snapshot, query string, category core.Category) []core.Scored {
	v := newView(s)
	results := slices.DeleteFunc(find(v, query, category), func(hit core.Scored) bool { return s.Hidden.Contains(hit.Result.ID) })
	for i := range results {
		decorate(v, &results[i].Result)
	}
	return results
}

// decorate adds Add Alias, Set Hotkey, and Hide to a result whose source
// allows them.
func decorate(v view, r *core.Result) {
	source, _, ok := core.ParseID(r.ID)
	// A row with typed text leaves these to the item's own row; there,
	// Hide would also be its second action, one Mod+Enter away.
	if !ok || carriesText(r) {
		return
	}
	traits := source.Traits()
	if traits.Aliasable {
		alias, _ := v.Aliases.Get(r.ID)
		label := "Add Alias…"
		if alias != "" {
			label = "Change Alias “" + alias + "”…"
		}
		r.Actions = append(r.Actions, core.Do(label, core.SetAlias{ID: r.ID, Alias: alias}))
	}
	// A hotkey runs the main action away from the launcher, where no one
	// could confirm it, so a result that asks first has none.
	if traits.Aliasable && len(r.Actions) > 0 && r.Actions[0].Confirm == "" {
		shortcut, _ := v.Hotkeys.Get(r.ID)
		label := "Set Hotkey…"
		if shortcut != "" {
			label = "Change Hotkey…"
		}
		r.Actions = append(r.Actions, core.Do(label, core.SetHotkey{ID: r.ID, Shortcut: shortcut}))
	}
	if traits.Hideable {
		r.Actions = append(r.Actions, core.Do("Hide from Results", core.Hide{ID: r.ID}))
	}
}

// carriesText reports whether a result runs its item with typed text, such
// as `Jira: ABC-12`: a keyword answer or a fallback. An alias or a hide is
// for the item, so it is set on the item's own row.
func carriesText(r *core.Result) bool {
	if len(r.Actions) == 0 {
		return false
	}
	switch a := r.Actions[0].Action.(type) {
	case core.OpenQuicklink:
		return a.Query != ""
	case core.CopySnippet:
		return a.Query != ""
	}
	return false
}

func find(v view, query string, category core.Category) []core.Scored {
	query = strings.TrimSpace(query)
	if query == "" {
		return unranked(browse(v, category))
	}
	m := match.New(query)
	if category == core.CategoryAll {
		return all(v, query, m)
	}
	// Each source cuts its own list, so it keeps room for the hidden
	// results that are left out after it.
	limit := CategoryLimit + v.Hidden.Len()
	var scored []core.Scored
	switch category {
	case core.CategoryApps:
		scored = v.Apps.Search(m, v.apps(), limit)
	case core.CategoryFiles:
		scored = v.Files.Search(m, v.files(), limit)
	case core.CategoryClipboard:
		scored = v.clips().Search(m, v.clipboard(), limit)
	case core.CategorySnippets:
		scored = v.Library.Search(m, v.library(), limit)
	case core.CategoryEmoji:
		scored = v.Emoji.Search(m, v.emoji(), limit)
	case core.CategorySystem:
		scored = system.Search(m, v.system())
	}
	scored = append(scored, aliasHits(v, m, category)...)
	scored = slices.DeleteFunc(scored, func(hit core.Scored) bool { return v.Hidden.Contains(hit.Result.ID) })
	results := byScore(scored, CategoryLimit)
	// A keyword with text, such as `jira ABC-12`, answers here as in All.
	// It has its item's own ID, so it takes the place of the name match.
	if category == core.CategorySnippets {
		if answer, ok := v.Library.KeywordAnswer(query); ok {
			results = slices.DeleteFunc(results, func(hit core.Scored) bool { return hit.Result.ID == answer.ID })
			results = slices.Insert(results, 0, core.Scored{Score: unscored, Result: answer})
			results = results[:min(len(results), CategoryLimit)]
		}
	}
	return results
}

// inCategory reports whether a result ID belongs in a category's search:
// in All, the sources the user keeps in All.
func inCategory(v view, id core.ID, category core.Category) bool {
	traits, ok := id.Traits()
	if !ok {
		return false
	}
	if category == core.CategoryAll {
		return v.Settings.InAll(traits.Category)
	}
	return traits.Category == category
}

// aliasHits are the results whose alias starts with the query. The same
// result may also match by name; the better score keeps it.
func aliasHits(v view, m *match.Matcher, category core.Category) []core.Scored {
	var out []core.Scored
	for _, p := range v.Aliases.pairs {
		if !inCategory(v, p.ID, category) {
			continue
		}
		score, ok := m.Alias(p.Value)
		if !ok {
			continue
		}
		if r, ok := resolve(v, p.ID); ok {
			out = append(out, core.Scored{Score: score + v.personal.Boost(p.ID), Result: r})
		}
	}
	return out
}

// browse lists a category for an empty query: pins first, then the
// category's own order.
func browse(v view, category core.Category) []core.Result {
	var results []core.Result
	for _, id := range v.Pins.ids {
		source, _, ok := core.ParseID(id)
		if !ok {
			continue
		}
		// A pinned clip is kept, not a favorite to launch: it heads the
		// Clipboard tab, and a search still finds it, but the start screen
		// leaves its rows to apps and other favorites.
		if category == core.CategoryAll && source == core.SourceClip || category != core.CategoryAll && source.Traits().Category != category {
			continue
		}
		if r, ok := resolve(v, id); ok {
			results = append(results, r)
		}
	}
	var rest []core.Result
	switch category {
	case core.CategoryAll:
		rest = used(v, suggestions, func(id core.ID, traits core.SourceTraits) bool {
			return !v.Pins.Contains(id) && traits.Suggestible && v.Settings.InAll(traits.Category)
		})
	case core.CategoryApps:
		rest = v.Apps.Browse(v.apps())
	case core.CategoryFiles:
		rest = used(v, CategoryLimit, func(_ core.ID, traits core.SourceTraits) bool {
			return traits.Category == core.CategoryFiles
		})
	case core.CategoryClipboard:
		rest = v.clips().Browse(v.clipboard())
	case core.CategorySnippets:
		rest = v.Library.Browse(v.library())
	case core.CategoryEmoji:
		// The catalog is too long to list whole, so it keeps room for the
		// hidden emoji that are left out below.
		rest = v.Emoji.Browse(v.emoji(), CategoryLimit+v.Hidden.Len())
	case core.CategorySystem:
		rest = system.Browse(v.system())
	}
	for _, r := range rest {
		if !r.Pinned {
			results = append(results, r)
		}
	}
	results = slices.DeleteFunc(results, func(r core.Result) bool { return v.Hidden.Contains(r.ID) })
	return results[:min(len(results), CategoryLimit)]
}

// used lists up to limit results that the user ran, most used first, that
// keep says to keep and that still exist.
func used(v view, limit int, keep func(core.ID, core.SourceTraits) bool) []core.Result {
	var out []core.Result
	for _, id := range v.Usage.Ranked(v.Now.Unix()) {
		if len(out) == limit {
			break
		}
		traits, ok := id.Traits()
		if !ok || v.Hidden.Contains(id) || !keep(id, traits) {
			continue
		}
		if r, ok := resolve(v, id); ok {
			out = append(out, r)
		}
	}
	return out
}

// Resolve rebuilds the current result for a pinned, used, or aliased ID;
// ok is false when the item is gone.
func Resolve(s *Snapshot, id core.ID) (core.Result, bool) {
	return resolve(newView(s), id)
}

func resolve(v view, id core.ID) (core.Result, bool) {
	source, key, ok := core.ParseID(id)
	if !ok {
		return core.Result{}, false
	}
	switch source {
	case core.SourceApp:
		return v.Apps.Get(key, v.apps())
	case core.SourceFile:
		return v.Files.Get(key, v.files())
	case core.SourceClip:
		n, err := strconv.ParseInt(key, 10, 64)
		if err != nil {
			return core.Result{}, false
		}
		return v.clips().Get(n, v.clipboard())
	case core.SourceSnippet, core.SourceLink:
		n, err := strconv.ParseInt(key, 10, 64)
		if err != nil {
			return core.Result{}, false
		}
		r, ok := v.Library.Get(n, v.library())
		// A snippet's ID never finds a quicklink, nor the other way round.
		if !ok || r.ID != id {
			return core.Result{}, false
		}
		return r, true
	case core.SourceEmoji:
		return v.Emoji.Get(key, v.emoji())
	case core.SourceSystem:
		return system.Get(key, v.system())
	}
	return core.Result{}, false
}

// ranked is one hit of an All search with its tier: order is the place of
// its source in the list.
type ranked struct {
	hit   core.Scored
	order int
}

// all searches every source the user keeps in All: instant answers, then
// name matches across sources, then fuzzy matches, then a web search unless
// a web keyword already answered. Inside each tier, sources keep a fixed
// order so that, for example, an app named like the query beats an emoji
// code.
func all(v view, query string, m *match.Matcher) []core.Scored {
	// `=` asks for the calculation history and nothing else.
	if history, ok := v.Calculations.Answer(query); ok {
		return unranked(history)
	}
	// So does `?`, for the list of keywords.
	if list, ok := v.Library.Keywords(query, keywordEngines); ok {
		return unranked(list)
	}
	results := unranked(answers(v, query))
	keywordSearch := slices.ContainsFunc(results, func(hit core.Scored) bool { return hit.Result.Kind == core.KindWebSearch })

	// A whole alias goes first; an alias the query only starts ranks with
	// its result's source, as in that source's tab.
	var exact, partial []core.Scored
	for _, hit := range aliasHits(v, m, core.CategoryAll) {
		if hit.Score >= match.ExactAlias {
			exact = append(exact, hit)
		} else {
			partial = append(partial, hit)
		}
	}
	limit := AllLimit + v.Hidden.Len()
	sources := []struct {
		category core.Category
		search   func() []core.Scored
	}{
		{core.CategoryApps, func() []core.Scored { return v.Apps.Search(m, v.apps(), limit) }},
		{core.CategorySystem, func() []core.Scored { return system.Search(m, v.system()) }},
		{core.CategorySnippets, func() []core.Scored { return v.Library.Search(m, v.library(), limit) }},
		{core.CategoryFiles, func() []core.Scored { return v.Files.Search(m, v.files(), limit) }},
		{core.CategoryClipboard, func() []core.Scored { return v.clips().Search(m, v.clipboard(), limit) }},
		{core.CategoryEmoji, func() []core.Scored { return v.Emoji.Search(m, v.emoji(), limit) }},
	}
	var hits []ranked
	for _, hit := range exact {
		hits = append(hits, ranked{hit, 0})
	}
	for i, source := range sources {
		// A source the user keeps out of All is not searched at all.
		if !v.Settings.InAll(source.category) {
			continue
		}
		for _, hit := range source.search() {
			hits = append(hits, ranked{hit, i + 1})
		}
		for _, hit := range partial {
			if traits, _ := hit.Result.ID.Traits(); traits.Category == source.category {
				hits = append(hits, ranked{hit, i + 1})
			}
		}
	}
	slices.SortStableFunc(hits, func(a, b ranked) int {
		aWeak, bWeak := a.hit.Score < match.Strong, b.hit.Score < match.Strong
		switch {
		case aWeak != bWeak:
			if aWeak {
				return 1
			}
			return -1
		case a.order != b.order:
			return a.order - b.order
		case a.hit.Score != b.hit.Score:
			if a.hit.Score > b.hit.Score {
				return -1
			}
			return 1
		}
		return 0
	})

	// A keyword answer has its snippet's or quicklink's own ID, and a
	// result may match by name and by alias; keep only the first of each
	// ID, so no two rows share one. A keyword answer carries the typed text.
	answered := map[core.ID]bool{}
	for _, r := range results {
		answered[r.Result.ID] = true
	}
	seen := map[core.ID]bool{}
	for id := range answered {
		seen[id] = true
	}
	var names []core.Scored
	for _, h := range hits {
		id := h.hit.Result.ID
		if !v.Hidden.Contains(id) && !seen[id] {
			seen[id] = true
			names = append(names, h.hit)
		}
	}

	// Quicklinks that can open the query, unless the user already chose a
	// keyword, such as `g` or a quicklink's own, or keeps snippets out of
	// All. A quicklink that shows as a name match is not offered again.
	chose := slices.ContainsFunc(results, func(hit core.Scored) bool {
		k := hit.Result.Kind
		return k == core.KindWebSearch || k == core.KindQuicklink || k == core.KindSnippet
	})
	var candidates []core.Result
	if !chose && v.Settings.InAll(core.CategorySnippets) {
		for _, link := range v.Library.Fallbacks(query, v.library(), v.Now, v.Home) {
			if !v.Hidden.Contains(link.ID) && !answered[link.ID] {
				candidates = append(candidates, link)
			}
		}
	}
	fallback := 0
	if !keywordSearch {
		fallback = 1
	}
	roomFor := func(links int) int { return max(AllLimit-(len(results)+fallback+links), 0) }
	// The links take room from the name matches, and the name matches that
	// show decide which links are left; fewer links leave more room, so
	// this settles within a few steps.
	links := min(quicklinkFallbacks, len(candidates))
	var room int
	var offered []core.Result
	for {
		room = roomFor(links)
		shown := map[core.ID]bool{}
		for _, hit := range names[:min(room, len(names))] {
			shown[hit.Result.ID] = true
		}
		offered = offered[:0]
		for _, link := range candidates {
			if len(offered) == quicklinkFallbacks {
				break
			}
			if !shown[link.ID] {
				offered = append(offered, link)
			}
		}
		if len(offered) == links {
			break
		}
		links = len(offered)
	}
	results = append(results, names[:min(room, len(names))]...)
	if !keywordSearch {
		results = append(results, core.Scored{Score: unscored, Result: web.Fallback(query, web.SearchEngine(v.Settings.SearchEngine))})
	}
	return append(results, unranked(offered)...)
}

// answers are the results computed from the query itself rather than
// looked up in an index, in the order they show.
func answers(v view, query string) []core.Result {
	var out []core.Result
	add := func(r core.Result, ok bool) {
		if ok {
			out = append(out, r)
		}
	}
	expression := calc.Expression(query)
	// A nil *currency.Rates in the interface would not be a nil Rates.
	var rates calc.Rates
	if v.Rates != nil {
		rates = *v.Rates
	}
	add(calc.Answer(expression, rates))
	if !v.Settings.CurrencyRatesEnabled {
		add(calc.RatesOffAnswer(expression))
	}
	out = append(out, datetime.Answers(query, v.Now)...)
	// A failed read of the random source leaves the passwords out.
	if passwords, err := password.Answers(query, v.Random); err == nil {
		out = append(out, passwords...)
	}
	add(cleanurl.Answer(query))
	if r, ok := color.Answer(query); ok {
		out = append(out, r)
	} else {
		add(unixtime.Answer(query, v.Now))
	}
	// The user's own keyword goes before a web keyword it shares, such as
	// `w`.
	add(v.Library.KeywordAnswer(query))
	add(web.Answer(query))
	return out
}

// byScore keeps the best limit hits, each ID once, at its best score. Equal
// scores keep their order.
func byScore(scored []core.Scored, limit int) []core.Scored {
	slices.SortStableFunc(scored, func(a, b core.Scored) int {
		switch {
		case a.Score > b.Score:
			return -1
		case a.Score < b.Score:
			return 1
		}
		return 0
	})
	seen := map[core.ID]bool{}
	var out []core.Scored
	for _, hit := range scored {
		if len(out) == limit {
			break
		}
		if !seen[hit.Result.ID] {
			seen[hit.Result.ID] = true
			out = append(out, hit)
		}
	}
	return out
}

func unranked(results []core.Result) []core.Scored {
	out := make([]core.Scored, len(results))
	for i, r := range results {
		out[i] = core.Scored{Score: unscored, Result: r}
	}
	return out
}
