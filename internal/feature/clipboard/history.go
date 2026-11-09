package clipboard

import (
	"fmt"
	"strings"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// Personal is what ranking knows about the user. Clipboard entries are not
// ranked by use, but they can be pinned; a nil Personal means no pins.
type Personal interface {
	// Boost is the usage bonus of a result.
	Boost(id core.ID) uint32
	Pinned(id core.ID) bool
}

// Context is how one search shows its results.
type Context struct {
	Personal Personal
	// Now is the Unix time that ages are measured from.
	Now int64
}

func (c Context) pinned(id core.ID) bool { return c.Personal != nil && c.Personal.Pinned(id) }

type indexed struct {
	Entry
	// haystack is the title and the text in the form matching compares.
	haystack string
}

// History is the saved entries, newest first.
type History struct {
	entries []indexed
}

// NewHistory indexes entries, which come newest first.
func NewHistory(entries []Entry) *History {
	h := &History{entries: make([]indexed, len(entries))}
	for i, e := range entries {
		h.entries[i] = indexed{e, match.Normalize(e.Title + "\n" + e.Text)}
	}
	return h
}

// Search finds the entries that hold every word of the query, newest first.
// Newer entries score higher, and clipboard text never outranks a strong
// name match elsewhere.
func (h *History) Search(m *match.Matcher, ctx Context, limit int) []core.Scored {
	words := strings.Split(m.Query(), " ")
	var out []core.Scored
	for i := range h.entries {
		if len(out) == limit {
			break
		}
		e := &h.entries[i]
		if !containsAll(e.haystack, words) {
			continue
		}
		out = append(out, core.Scored{Score: uint32(max(1_000-len(out), 0)), Result: result(&e.Entry, ctx)})
	}
	return out
}

func containsAll(haystack string, words []string) bool {
	for _, word := range words {
		if !strings.Contains(haystack, word) {
			return false
		}
	}
	return true
}

// Browse lists every entry, newest first.
func (h *History) Browse(ctx Context) []core.Result {
	out := make([]core.Result, len(h.entries))
	for i := range h.entries {
		out[i] = result(&h.entries[i].Entry, ctx)
	}
	return out
}

// Get is the result of the entry with this ID.
func (h *History) Get(id int64, ctx Context) (core.Result, bool) {
	for i := range h.entries {
		if h.entries[i].ID == id {
			return result(&h.entries[i].Entry, ctx), true
		}
	}
	return core.Result{}, false
}

func result(e *Entry, ctx Context) core.Result {
	resultID := core.SourceClip.ID(fmt.Sprint(e.ID))
	label, symbol := "Text", core.SymbolText
	switch e.Kind {
	case KindImage:
		label, symbol = "Image", core.SymbolImage
	case KindFiles:
		label, symbol = "Files", core.SymbolFiles
	}
	pinned := ctx.pinned(resultID)
	pin := core.Do("Pin", core.Pin{ID: resultID})
	if pinned {
		pin = core.Do("Unpin", core.Unpin{ID: resultID})
	}
	// Mod+Enter runs the second action and may wait for newer results, so
	// Delete, which cannot be undone, never takes that place.
	actions := []core.ResultAction{core.Do("Copy", core.CopyClip{ID: e.ID}), pin}
	// Text is saved plain already.
	if e.Kind == KindFiles {
		actions = append(actions, core.Do("Copy as Plain Text", core.CopyClipText{ID: e.ID}))
	}
	actions = append(actions, core.Do("Delete", core.DeleteClip{ID: e.ID}))
	return core.Result{
		ID:       resultID,
		Kind:     core.KindClipboard,
		Title:    e.Title,
		Subtitle: label + " · " + ago(ctx.Now-e.CopiedAt),
		Icon:     core.SymbolIcon(symbol),
		Actions:  actions,
		Pinned:   pinned,
	}
}

func ago(seconds int64) string {
	plural := func(n int64, unit string) string {
		if n == 1 {
			return fmt.Sprintf("%d %s ago", n, unit)
		}
		return fmt.Sprintf("%d %ss ago", n, unit)
	}
	switch seconds = max(seconds, 0); {
	case seconds < 60:
		return "Just now"
	case seconds < 3_600:
		return plural(seconds/60, "minute")
	case seconds < 86_400:
		return plural(seconds/3_600, "hour")
	}
	return plural(seconds/86_400, "day")
}
