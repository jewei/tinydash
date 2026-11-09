package search

import (
	"cmp"
	"errors"
	"maps"
	"slices"
	"strings"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// The personal lists are values: a method that changes one returns a new
// value and leaves the old one as it was, because published snapshots hold
// them and readers never lock.

// Limits of the personal lists. The engine checks them before it adds.
const (
	// MaxUsage is the most result IDs whose usage is remembered, in memory
	// and in the database.
	MaxUsage = 1000
	// MaxPins is the most pins.
	MaxPins = 100
	// MaxHidden is the most results the user may hide.
	MaxHidden = 500
	// MaxAliases is the most aliases the user may set.
	MaxAliases = 500
	// MaxHotkeys is the most hotkeys. Each holds a system-wide key
	// combination, so the list stays short.
	MaxHotkeys = 50
	// maxAlias is the longest alias, in characters.
	maxAlias = 32
)

const day = 86_400

// Usage is how often and how lately each result ran, by result ID.
type Usage struct {
	entries map[core.ID]core.Use
}

// NewUsage holds a copy of entries.
func NewUsage(entries map[core.ID]core.Use) Usage {
	return Usage{maps.Clone(entries)}
}

// Record counts one use at now (Unix seconds) and returns the new usage and
// the entry. Past MaxUsage IDs, the least recently used goes; of equally old
// ones, the largest ID, as the store drops them.
func (u Usage) Record(id core.ID, now int64) (Usage, core.Use) {
	next := make(map[core.ID]core.Use, len(u.entries)+1)
	maps.Copy(next, u.entries)
	used := next[id]
	if used.Count < ^uint32(0) {
		used.Count++
	}
	// A clock set back must not make the entry older, as on disk.
	used.LastUsed = max(used.LastUsed, now)
	next[id] = used
	if len(next) > MaxUsage {
		var oldest core.ID
		first := true
		for other, entry := range next {
			if first || entry.LastUsed < next[oldest].LastUsed || entry.LastUsed == next[oldest].LastUsed && other > oldest {
				oldest, first = other, false
			}
		}
		delete(next, oldest)
	}
	return Usage{next}, used
}

// Remove forgets the usage of id.
func (u Usage) Remove(id core.ID) Usage {
	if _, ok := u.entries[id]; !ok {
		return u
	}
	next := maps.Clone(u.entries)
	delete(next, id)
	return Usage{next}
}

// Bonus is the ranking bonus of id at now, at most 1,000: up to 500 for
// frequency and up to 500 for recency. A strong name match (2,700 or more)
// always outranks usage alone.
func (u Usage) Bonus(id core.ID, now int64) uint32 {
	entry, ok := u.entries[id]
	if !ok {
		return 0
	}
	frequency := min(entry.Count, 20) * 25
	days := max(now-entry.LastUsed, 0) / day
	return frequency + uint32(500/(days+1))
}

// Ranked lists the IDs from most to least used, with ties broken by
// recency, then by ID.
func (u Usage) Ranked(now int64) []core.ID {
	ids := slices.Collect(maps.Keys(u.entries))
	slices.SortFunc(ids, func(a, b core.ID) int {
		return cmp.Or(
			cmp.Compare(u.Bonus(b, now), u.Bonus(a, now)),
			cmp.Compare(u.entries[b].LastUsed, u.entries[a].LastUsed),
			cmp.Compare(a, b),
		)
	})
	return ids
}

// Pins are the pinned result IDs, oldest pin first.
type Pins struct {
	ids []core.ID
}

// NewPins holds a copy of ids.
func NewPins(ids []core.ID) Pins { return Pins{slices.Clone(ids)} }

// Contains reports whether id is pinned.
func (p Pins) Contains(id core.ID) bool { return slices.Contains(p.ids, id) }

// IDs lists the pins, oldest first.
func (p Pins) IDs() []core.ID { return slices.Clone(p.ids) }

// Len is the number of pins.
func (p Pins) Len() int { return len(p.ids) }

// Add pins id last, unless it is pinned already.
func (p Pins) Add(id core.ID) Pins {
	if p.Contains(id) {
		return p
	}
	return Pins{append(slices.Clip(p.ids), id)}
}

// Remove unpins id.
func (p Pins) Remove(id core.ID) Pins {
	if !p.Contains(id) {
		return p
	}
	return Pins{slices.DeleteFunc(slices.Clone(p.ids), func(pin core.ID) bool { return pin == id })}
}

// Hidden are the result IDs the user hid; search leaves them out.
type Hidden struct {
	ids map[core.ID]struct{}
}

// NewHidden holds ids.
func NewHidden(ids ...core.ID) Hidden {
	set := make(map[core.ID]struct{}, len(ids))
	for _, id := range ids {
		set[id] = struct{}{}
	}
	return Hidden{set}
}

// Contains reports whether id is hidden.
func (h Hidden) Contains(id core.ID) bool {
	_, ok := h.ids[id]
	return ok
}

// Len is the number of hidden results.
func (h Hidden) Len() int { return len(h.ids) }

// IDs lists the hidden results in order.
func (h Hidden) IDs() []core.ID { return slices.Sorted(maps.Keys(h.ids)) }

// Add hides id.
func (h Hidden) Add(id core.ID) Hidden {
	if h.Contains(id) {
		return h
	}
	next := make(map[core.ID]struct{}, len(h.ids)+1)
	maps.Copy(next, h.ids)
	next[id] = struct{}{}
	return Hidden{next}
}

// Remove shows id again.
func (h Hidden) Remove(id core.ID) Hidden {
	if !h.Contains(id) {
		return h
	}
	next := maps.Clone(h.ids)
	delete(next, id)
	return Hidden{next}
}

// Pair is a result ID and its alias or hotkey.
type Pair struct {
	ID    core.ID
	Value string
}

// pairs is a list of pairs sorted by ID, at most one per ID.
type pairs []Pair

func newPairs(byID map[core.ID]string) pairs {
	out := make(pairs, 0, len(byID))
	for id, value := range byID {
		out = append(out, Pair{id, value})
	}
	slices.SortFunc(out, func(a, b Pair) int { return cmp.Compare(a.ID, b.ID) })
	return out
}

func (p pairs) get(id core.ID) (string, bool) {
	i, ok := slices.BinarySearchFunc(p, id, func(e Pair, id core.ID) int { return cmp.Compare(e.ID, id) })
	if !ok {
		return "", false
	}
	return p[i].Value, true
}

func (p pairs) set(id core.ID, value string) pairs {
	i, ok := slices.BinarySearchFunc(p, id, func(e Pair, id core.ID) int { return cmp.Compare(e.ID, id) })
	next := slices.Clone(p)
	if ok {
		next[i].Value = value
		return next
	}
	return slices.Insert(next, i, Pair{id, value})
}

func (p pairs) remove(id core.ID) pairs {
	i, ok := slices.BinarySearchFunc(p, id, func(e Pair, id core.ID) int { return cmp.Compare(e.ID, id) })
	if !ok {
		return p
	}
	return slices.Delete(slices.Clone(p), i, i+1)
}

// Aliases are the user's own short names for results. Each result has at
// most one alias, and each alias names one result.
type Aliases struct {
	pairs pairs
}

// NewAliases holds the aliases by result ID.
func NewAliases(byID map[core.ID]string) Aliases { return Aliases{newPairs(byID)} }

// Get is the alias of id.
func (a Aliases) Get(id core.ID) (string, bool) { return a.pairs.get(id) }

// Owner is the result ID that has this alias.
func (a Aliases) Owner(alias string) (core.ID, bool) {
	for _, p := range a.pairs {
		if p.Value == alias {
			return p.ID, true
		}
	}
	return "", false
}

// Len is the number of aliases.
func (a Aliases) Len() int { return len(a.pairs) }

// All lists the aliases by result ID.
func (a Aliases) All() []Pair { return slices.Clone(a.pairs) }

// Set gives id the alias.
func (a Aliases) Set(id core.ID, alias string) Aliases { return Aliases{a.pairs.set(id, alias)} }

// Remove removes the alias of id.
func (a Aliases) Remove(id core.ID) Aliases { return Aliases{a.pairs.remove(id)} }

// ErrAlias refuses an alias that is not one short word.
var ErrAlias = errors.New("An alias is one word of up to 32 characters.")

// CleanAlias puts an alias in the form search compares: one lowercase word
// of up to 32 characters. Empty means "no alias".
func CleanAlias(text string) (string, error) {
	alias := match.Normalize(text)
	if strings.Contains(alias, " ") || utf8.RuneCountInString(alias) > maxAlias {
		return "", ErrAlias
	}
	return alias, nil
}

// Hotkeys are key combinations that run a result from any app, by result
// ID, as accelerator text such as `Control+Alt+KeyS`. Each result has at
// most one, and each combination runs one result.
type Hotkeys struct {
	pairs pairs
}

// NewHotkeys holds the hotkeys by result ID.
func NewHotkeys(byID map[core.ID]string) Hotkeys { return Hotkeys{newPairs(byID)} }

// Get is the hotkey of id.
func (h Hotkeys) Get(id core.ID) (string, bool) { return h.pairs.get(id) }

// Len is the number of hotkeys.
func (h Hotkeys) Len() int { return len(h.pairs) }

// All lists the hotkeys by result ID.
func (h Hotkeys) All() []Pair { return slices.Clone(h.pairs) }

// Set gives id the hotkey.
func (h Hotkeys) Set(id core.ID, shortcut string) Hotkeys { return Hotkeys{h.pairs.set(id, shortcut)} }

// Remove removes the hotkey of id.
func (h Hotkeys) Remove(id core.ID) Hotkeys { return Hotkeys{h.pairs.remove(id)} }

// personal ranks with the snapshot's usage and pins at its clock.
type personal struct {
	usage Usage
	pins  Pins
	now   int64
}

func (p personal) Boost(id core.ID) uint32 { return p.usage.Bonus(id, p.now) }
func (p personal) Pinned(id core.ID) bool  { return p.pins.Contains(id) }
