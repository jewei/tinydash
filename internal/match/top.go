package match

import (
	"cmp"
	"slices"
)

// Hit is an item with its score: higher ranks first.
type Hit[T any] struct {
	Score uint32
	Item  T
}

// Top keeps the limit best hits, best first. Equal scores keep the order
// they came in, so ties rank the same way on every search.
func Top[T any](hits []Hit[T], limit int) []Hit[T] {
	slices.SortStableFunc(hits, func(a, b Hit[T]) int { return cmp.Compare(b.Score, a.Score) })
	if len(hits) > limit {
		hits = hits[:limit]
	}
	return hits
}
