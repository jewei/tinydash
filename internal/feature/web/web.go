// Package web answers web searches: `g rust`, `yt lo-fi`, `gh tauri`, and
// the fallback search at the end of an All search without such a keyword.
// Nothing is sent until the user opens the result.
package web

import (
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
)

// SearchEngine is the engine of the fallback search, as settings.json names it.
type SearchEngine string

const (
	Google     SearchEngine = "google"
	DuckDuckGo SearchEngine = "duckDuckGo"
	Bing       SearchEngine = "bing"
	Brave      SearchEngine = "brave"
)

// Engine is a site that keyword searches can open.
type Engine struct {
	Name     string
	Keywords []string
	// Template has {query} where the encoded search text goes.
	Template string
}

var engines = []Engine{
	{"Google", []string{"g", "google"}, "https://www.google.com/search?q={query}"},
	{"DuckDuckGo", []string{"ddg", "duckduckgo"}, "https://duckduckgo.com/?q={query}"},
	{"Bing", []string{"bing"}, "https://www.bing.com/search?q={query}"},
	{"Brave", []string{"brave"}, "https://search.brave.com/search?q={query}"},
	{"YouTube", []string{"yt", "youtube"}, "https://www.youtube.com/results?search_query={query}"},
	{"GitHub", []string{"gh", "github"}, "https://github.com/search?q={query}"},
	{"Wikipedia", []string{"w", "wiki", "wikipedia"}, "https://en.wikipedia.org/w/index.php?search={query}"},
}

// fallbackEngines maps each SearchEngine to its row in engines.
var fallbackEngines = map[SearchEngine]int{Google: 0, DuckDuckGo: 1, Bing: 2, Brave: 3}

// Engines lists every engine with its keywords, shortest first, for the `?` list.
func Engines() []Engine {
	out := make([]Engine, len(engines))
	for i, e := range engines {
		e.Keywords = append([]string(nil), e.Keywords...)
		out[i] = e
	}
	return out
}

// Answer is a search for an engine keyword followed by text. A bare keyword
// is not a search, so `gh` can still find an app.
func Answer(query string) (core.Result, bool) {
	query = strings.TrimSpace(query)
	at := strings.IndexFunc(query, unicode.IsSpace)
	if at < 0 {
		return core.Result{}, false
	}
	_, size := utf8.DecodeRuneInString(query[at:])
	keyword, text := strings.ToLower(query[:at]), strings.TrimSpace(query[at+size:])
	for _, e := range engines {
		for _, k := range e.Keywords {
			if k == keyword && text != "" {
				return result(e, text), true
			}
		}
	}
	return core.Result{}, false
}

// Fallback searches the whole query with the chosen engine; an unknown
// engine is Google, the default.
func Fallback(query string, engine SearchEngine) core.Result {
	return result(engines[fallbackEngines[engine]], strings.TrimSpace(query))
}

func result(e Engine, text string) core.Result {
	url := Fill(e.Template, text)
	return core.Result{
		ID:       core.ID("web:" + e.Name + ":" + text),
		Kind:     core.KindWebSearch,
		Title:    "Search " + e.Name + " for “" + text + "”",
		Subtitle: url,
		Icon:     core.SymbolIcon(core.SymbolGlobe),
		Actions: []core.ResultAction{
			core.Do("Open in Browser", core.OpenURL{URL: url}),
			core.Do("Copy URL", core.Copy{Text: url}),
		},
	}
}

// Fill replaces {query} in a template with the text, encoded with Encode.
func Fill(template, text string) string {
	return strings.ReplaceAll(template, "{query}", Encode(text))
}

// Encode percent-encodes text so it stays one value in a URL path or query
// string: a space becomes %20, never +.
func Encode(text string) string {
	var b strings.Builder
	for i := 0; i < len(text); i++ {
		c := text[i]
		if 'a' <= c && c <= 'z' || 'A' <= c && c <= 'Z' || '0' <= c && c <= '9' || strings.IndexByte("-._~", c) >= 0 {
			b.WriteByte(c)
		} else {
			fmt.Fprintf(&b, "%%%02X", c)
		}
	}
	return b.String()
}
