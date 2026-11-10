package match

import (
	"cmp"
	"container/heap"
	"slices"
)

// Hit is an item with its score: higher ranks first.
type Hit[T any] struct {
	Score uint32
	Item  T
}

// Top keeps the limit best hits, best first. Equal scores keep the order
// they came in, so ties rank the same way on every search. It is the
// first limit hits of a stable sort, without sorting them all: a short
// query matches most of 55,000 names, and only the best few are shown.
func Top[T any](hits []Hit[T], limit int) []Hit[T] {
	if limit <= 0 {
		return hits[:0]
	}
	if len(hits) <= limit {
		slices.SortStableFunc(hits, func(a, b Hit[T]) int { return cmp.Compare(b.Score, a.Score) })
		return hits
	}
	// worst holds the positions of the best limit hits so far, the worst
	// of them at the root. Of two equal scores the later one is worse, so
	// a later tie never displaces an earlier hit.
	w := &worst[T]{hits: hits, at: make([]int, limit)}
	for i := range limit {
		w.at[i] = i
	}
	heap.Init(w)
	for i := limit; i < len(hits); i++ {
		if hits[i].Score > hits[w.at[0]].Score {
			w.at[0] = i
			heap.Fix(w, 0)
		}
	}
	slices.SortFunc(w.at, func(a, b int) int {
		return cmp.Or(cmp.Compare(hits[b].Score, hits[a].Score), cmp.Compare(a, b))
	})
	out := make([]Hit[T], limit)
	for i, at := range w.at {
		out[i] = hits[at]
	}
	return out
}

// worst is a heap of positions in hits, the worst hit first.
type worst[T any] struct {
	hits []Hit[T]
	at   []int
}

func (w *worst[T]) Len() int { return len(w.at) }
func (w *worst[T]) Less(i, j int) bool {
	a, b := w.hits[w.at[i]], w.hits[w.at[j]]
	return a.Score < b.Score || a.Score == b.Score && w.at[i] > w.at[j]
}
func (w *worst[T]) Swap(i, j int) { w.at[i], w.at[j] = w.at[j], w.at[i] }
func (w *worst[T]) Push(any)      { panic("worst has a fixed size") }
func (w *worst[T]) Pop() any      { panic("worst has a fixed size") }
