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

// Top keeps the limit best hits, best first. Hits with equal scores come in
// the order the Rust app gave them: it chose the best with Rust's
// select_nth_unstable, ported below, then sorted them stably.
func Top[T any](hits []Hit[T], limit int) []Hit[T] {
	if len(hits) > limit {
		selectNth(hits, limit)
		hits = hits[:limit]
	}
	slices.SortStableFunc(hits, func(a, b Hit[T]) int { return cmp.Compare(b.Score, a.Score) })
	return hits
}

// better is the order Top selects by: a higher score is "less".
func better[T any](a, b Hit[T]) bool { return a.Score > b.Score }

type lessFunc[T any] func(a, b Hit[T]) bool

// selectNth is Rust 1.99's slice::select_nth_unstable_by_key: it puts the
// item that belongs at index there, better items before it, others after.
func selectNth[T any](v []Hit[T], index int) {
	switch index {
	case len(v) - 1:
		i := maxIndex(v, better[T])
		v[i], v[index] = v[index], v[i]
	case 0:
		i := minIndex(v, better[T])
		v[i], v[0] = v[0], v[i]
	default:
		partitionAtIndex(v, index)
	}
}

const insertionSortThreshold = 16

func partitionAtIndex[T any](v []Hit[T], index int) {
	less := better[T]
	limit := 16
	var ancestor *Hit[T]
	for {
		if len(v) <= insertionSortThreshold {
			insertionSort(v, less)
			return
		}
		if limit == 0 {
			medianOfMedians(v, less, index)
			return
		}
		limit--
		pivot := choosePivot(v, less)
		// Equal to an earlier pivot: everything up to it is in place.
		if ancestor != nil && !less(*ancestor, v[pivot]) {
			mid := partition(v, pivot, func(a, b Hit[T]) bool { return !less(b, a) }) + 1
			if mid > index {
				return
			}
			v, index, ancestor = v[mid:], index-mid, nil
			continue
		}
		mid := partition(v, pivot, less)
		switch {
		case mid < index:
			p := v[mid]
			v, index, ancestor = v[mid+1:], index-mid-1, &p
		case mid > index:
			v = v[:mid]
		default:
			return
		}
	}
}

func minIndex[T any](v []Hit[T], less lessFunc[T]) int {
	best := 0
	for i := range v {
		if less(v[i], v[best]) {
			best = i
		}
	}
	return best
}

func maxIndex[T any](v []Hit[T], less lessFunc[T]) int {
	best := 0
	for i := range v {
		if i > 0 && less(v[best], v[i]) {
			best = i
		}
	}
	return best
}

func insertionSort[T any](v []Hit[T], less lessFunc[T]) {
	for i := 1; i < len(v); i++ {
		tail := v[i]
		j := i
		for j > 0 && less(tail, v[j-1]) {
			v[j] = v[j-1]
			j--
		}
		v[j] = tail
	}
}

func choosePivot[T any](v []Hit[T], less lessFunc[T]) int {
	n8 := len(v) / 8
	a, b, c := 0, n8*4, n8*7
	if len(v) < 64 {
		return median3(v, less, a, b, c)
	}
	return median3Rec(v, less, a, b, c, n8)
}

func median3Rec[T any](v []Hit[T], less lessFunc[T], a, b, c, n int) int {
	if n*8 >= 64 {
		n8 := n / 8
		a = median3Rec(v, less, a, a+n8*4, a+n8*7, n8)
		b = median3Rec(v, less, b, b+n8*4, b+n8*7, n8)
		c = median3Rec(v, less, c, c+n8*4, c+n8*7, n8)
	}
	return median3(v, less, a, b, c)
}

func median3[T any](v []Hit[T], less lessFunc[T], a, b, c int) int {
	x, y := less(v[a], v[b]), less(v[a], v[c])
	if x != y {
		return a
	}
	if less(v[b], v[c]) != x {
		return c
	}
	return b
}

// partition moves the pivot to its place and returns it; items less than
// the pivot go before it. It is Rust's cyclic branchless Lomuto partition,
// whose moves decide where equal items end up.
func partition[T any](v []Hit[T], pivot int, less lessFunc[T]) int {
	v[0], v[pivot] = v[pivot], v[0]
	p, rest := v[0], v[1:]
	lt := 0
	if len(rest) > 0 {
		saved, gap := rest[0], 0
		for r := 1; r <= len(rest); r++ {
			x := saved
			if r < len(rest) {
				x = rest[r]
			}
			isLess := less(x, p)
			rest[gap] = rest[lt]
			rest[lt] = x
			gap = r
			if isLess {
				lt++
			}
		}
	}
	v[0], v[lt] = v[lt], v[0]
	return lt
}

func medianOfMedians[T any](v []Hit[T], less lessFunc[T], k int) {
	for {
		switch {
		case len(v) <= insertionSortThreshold:
			insertionSort(v, less)
			return
		case k == len(v)-1:
			i := maxIndex(v, less)
			v[i], v[k] = v[k], v[i]
			return
		case k == 0:
			i := minIndex(v, less)
			v[i], v[0] = v[0], v[i]
			return
		}
		p := medianOfNinthers(v, less)
		switch {
		case p == k:
			return
		case p > k:
			v = v[:p]
		default:
			v, k = v[p+1:], k-p-1
		}
	}
}

func medianOfNinthers[T any](v []Hit[T], less lessFunc[T]) int {
	n := len(v)
	frac := n / 1024
	switch {
	case n <= 1024:
		frac = n / 12
	case n <= 128*1024:
		frac = n / 64
	}
	pivot := frac / 2
	lo := n/2 - pivot
	hi := frac + lo
	gap := (n - 9*frac) / 4
	a, b := lo-4*frac-gap, hi+gap
	for i := lo; i < hi; i++ {
		ninther(v, less, a, i-frac, b, a+1, i, b+1, a+2, i+frac, b+2)
		a += 3
		b += 3
	}
	medianOfMedians(v[lo:lo+frac], less, pivot)
	return partition(v, lo+pivot, less)
}

func ninther[T any](v []Hit[T], less lessFunc[T], a, b, c, d, e, f, g, h, i int) {
	b = medianIndex(v, less, a, b, c)
	h = medianIndex(v, less, g, h, i)
	if less(v[h], v[b]) {
		b, h = h, b
	}
	if less(v[f], v[d]) {
		d, f = f, d
	}
	switch {
	case less(v[e], v[d]):
	case less(v[f], v[e]):
		d = f
	default:
		if less(v[e], v[b]) {
			v[e], v[b] = v[b], v[e]
		} else if less(v[h], v[e]) {
			v[e], v[h] = v[h], v[e]
		}
		return
	}
	if less(v[d], v[b]) {
		d = b
	} else if less(v[h], v[d]) {
		d = h
	}
	v[d], v[e] = v[e], v[d]
}

func medianIndex[T any](v []Hit[T], less lessFunc[T], a, b, c int) int {
	if less(v[c], v[a]) {
		a, c = c, a
	}
	if less(v[c], v[b]) {
		return c
	}
	if less(v[b], v[a]) {
		return a
	}
	return b
}
