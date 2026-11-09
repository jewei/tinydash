package match

import "slices"

// The scoring of nucleo-matcher 0.3.1 for the calls TinyDash makes: one atom
// per query word, case ignored, Smart normalization, the default
// configuration, and the fuzzy or substring kind. It keeps nucleo's quirks,
// such as a substring search that skips the last place a word could start,
// because the Rust app's ranking came from them.

// atom is one query word.
type atom struct {
	needle    text
	normalize bool // match Latin letters with marks as ASCII
	substring bool
}

func newAtom(word string, substring bool) atom {
	a := atom{substring: substring, normalize: true}
	if isASCII(word) {
		runes := make([]rune, len(word))
		for i := range len(word) {
			runes[i] = normalizeRune(rune(word[i]), true, false)
		}
		a.needle = text{runes, true}
		return a
	}
	starts := graphemeStarts(word)
	for i, r := range starts {
		starts[i] = fold(r)
		// A word with a mark asks for that mark: match it as written.
		a.normalize = a.normalize && latin(starts[i]) == starts[i]
	}
	a.needle = text{starts, false}
	return a
}

func isASCII(s string) bool {
	for i := range len(s) {
		if s[i] >= 0x80 {
			return false
		}
	}
	return true
}

func (a atom) score(hay text) (uint16, bool) {
	n, h := a.needle.runes, hay.runes
	switch {
	case len(n) > len(h):
		return 0, false
	case len(n) == 0:
		return 0, true
	case len(n) == len(h):
		return a.exact(hay, 0, len(h))
	case hay.ascii && !a.needle.ascii:
		return 0, false
	case a.substring:
		return a.findSubstring(hay)
	}
	return a.fuzzy(hay)
}

func (a atom) fuzzy(hay text) (uint16, bool) {
	n := a.needle.runes
	if hay.ascii {
		if len(n) == 1 {
			return a.substringOneASCII(hay)
		}
		start, greedyEnd, end, ok := a.prefilterASCII(hay)
		if !ok {
			return 0, false
		}
		if len(n) == end-start {
			return a.calculateScore(hay, start, greedyEnd), true
		}
		return a.optimal(hay, start, greedyEnd, end)
	}
	if len(n) == 1 {
		start, _, ok := a.prefilterUnicode(hay, true)
		if !ok {
			return 0, false
		}
		return a.substringOneUnicode(hay, start), true
	}
	start, end, ok := a.prefilterUnicode(hay, false)
	switch {
	case !ok:
		return 0, false
	case len(n) == end-start:
		return a.exact(hay, start, end)
	}
	return a.optimal(hay, start, start+1, end)
}

func (a atom) findSubstring(hay text) (uint16, bool) {
	n := a.needle.runes
	switch {
	case hay.ascii && len(n) == 1:
		return a.substringOneASCII(hay)
	case hay.ascii:
		return a.substringASCII(hay)
	case len(n) == 1:
		start, _, ok := a.prefilterUnicode(hay, true)
		if !ok {
			return 0, false
		}
		return a.substringOneUnicode(hay, start), true
	}
	start, _, ok := a.prefilterUnicode(hay, false)
	if !ok {
		return 0, false
	}
	return a.substringUnicode(hay, start)
}

func (a atom) norm(r rune, ascii bool) rune { return normalizeRune(r, ascii, a.normalize) }

func (a atom) classAndNorm(r rune, ascii bool) (rune, charClass) {
	return classAndNormalize(r, ascii, a.normalize)
}

func prevClass(hay text, i int) charClass {
	if i == 0 {
		return initialClass
	}
	return class(hay.runes[i-1], hay.ascii)
}

func (a atom) exact(hay text, start, end int) (uint16, bool) {
	n := a.needle.runes
	if len(n) != end-start || hay.ascii && !a.needle.ascii {
		return 0, false
	}
	for i, r := range hay.runes[start:end] {
		if a.norm(r, hay.ascii) != a.norm(n[i], a.needle.ascii) {
			return 0, false
		}
	}
	return a.calculateScore(hay, start, end), true
}

// calculateScore scores the match of the needle in hay[start:end], taking
// each needle rune at its first place.
func (a atom) calculateScore(hay text, start, end int) uint16 {
	n, h := a.needle.runes, hay.runes
	prev := prevClass(hay, start)
	c := class(h[start], hay.ascii)
	firstBonus := bonusFor(prev, c)
	score := scoreMatch + firstBonus*bonusFirstCharFactor
	prev = c
	next, want := 1, n[0]
	if len(n) > 1 {
		want, next = n[1], 2
	}
	inGap, consecutive := false, 1
	for _, r := range h[start+1 : end] {
		r, c := a.classAndNorm(r, hay.ascii)
		if r == want {
			bonus := bonusFor(prev, c)
			if consecutive != 0 {
				if bonus >= bonusBoundary && bonus > firstBonus {
					firstBonus = bonus
				}
				bonus = max(bonus, firstBonus, bonusConsecutive)
			} else {
				firstBonus = bonus
			}
			score += scoreMatch + bonus
			inGap = false
			consecutive++
			if next < len(n) {
				want = n[next]
				next++
			}
		} else {
			penalty := uint16(penaltyGapStart)
			if inGap {
				penalty = penaltyGapExtension
			}
			score = max(score, penalty) - penalty
			inGap = true
			consecutive = 0
		}
		prev = c
	}
	return score
}

// prefilterASCII finds where a match can start, where the greedy match
// ends, and where the last needle byte last appears.
func (a atom) prefilterASCII(hay text) (start, greedyEnd, end int, ok bool) {
	n, h := a.needle.runes, hay.runes
	start = indexFold(h[:len(h)-len(n)+1], n[0])
	if start < 0 {
		return 0, 0, 0, false
	}
	greedyEnd = start + 1
	for _, r := range n[1:] {
		i := indexFold(h[greedyEnd:], r)
		if i < 0 {
			return 0, 0, 0, false
		}
		greedyEnd += i + 1
	}
	end = greedyEnd + lastIndexFold(h[greedyEnd:], n[len(n)-1]) + 1
	return start, greedyEnd, end, true
}

func equalFoldASCII(b, r rune) bool {
	return b == r || 'a' <= r && r <= 'z' && b == r-asciiLetterCaseOffset
}

func indexFold(h []rune, r rune) int {
	for i, b := range h {
		if equalFoldASCII(b, r) {
			return i
		}
	}
	return -1
}

func lastIndexFold(h []rune, r rune) int {
	for i := len(h) - 1; i >= 0; i-- {
		if equalFoldASCII(h[i], r) {
			return i
		}
	}
	return -1
}

func (a atom) prefilterUnicode(hay text, onlyGreedy bool) (start, end int, ok bool) {
	n, h := a.needle.runes, hay.runes
	start = -1
	for i, r := range h[:len(h)-len(n)+1] {
		if a.norm(r, false) == n[0] {
			start = i
			break
		}
	}
	if start < 0 {
		return 0, 0, false
	}
	if onlyGreedy {
		return start, start + 1, len(h)-start >= len(n)
	}
	for end = len(h); end > start+1; end-- {
		if a.norm(h[end-1], false) == n[len(n)-1] {
			return start, end, end-start >= len(n)
		}
	}
	return 0, 0, false
}

func (a atom) substringOneASCII(hay text) (uint16, bool) {
	want := a.needle.runes[0]
	var best uint16
	for i, r := range hay.runes {
		if !equalFoldASCII(r, want) {
			continue
		}
		bonus := bonusFor(prevClass(hay, i), class(r, true))
		if score := bonus*bonusFirstCharFactor + scoreMatch; score > best {
			best = score
			if bonus >= bonusBoundaryWhite {
				break
			}
		}
	}
	return best, best > 0
}

// substringOneUnicode keeps nucleo's quirk: the class before a rune is the
// class of the last match, not of the rune before it.
func (a atom) substringOneUnicode(hay text, start int) uint16 {
	want := a.needle.runes[0]
	prev := prevClass(hay, start)
	var best uint16
	for _, r := range hay.runes[start:] {
		r, c := a.classAndNorm(r, false)
		if r != want {
			continue
		}
		bonus := bonusFor(prev, c)
		prev = c
		if score := bonus*bonusFirstCharFactor + scoreMatch; score > best {
			best = score
			if bonus >= bonusBoundaryWhite {
				break
			}
		}
	}
	return best
}

func (a atom) substringASCII(hay text) (uint16, bool) {
	n, h := a.needle.runes, hay.runes
	letter := -1
	for i, r := range n {
		if 'a' <= r && r <= 'z' {
			letter = i
			break
		}
	}
	var candidates []int
	switch {
	case letter == 0:
		for i, r := range h[:len(h)-len(n)+1] {
			if equalFoldASCII(r, n[0]) {
				candidates = append(candidates, i)
			}
		}
	case letter == 1:
		for i, r := range h[:len(h)-len(n)+1] {
			if r == n[0] {
				candidates = append(candidates, i)
			}
		}
	case letter > 1:
		// nucleo searches a haystack cut short by the needle's length less
		// the letter's place, so it misses a match near the end.
		candidates = occurrences(h[:len(h)-len(n)+letter], n)
	default:
		candidates = occurrences(h, n)
	}
	var best uint16
	bestAt := 0
	for _, i := range candidates {
		bonus := bonusFor(prevClass(hay, i), class(h[i], true))
		score := bonus*bonusFirstCharFactor + scoreMatch
		if score > best && a.restMatches(hay, i) {
			best, bestAt = score, i
			if bonus >= bonusBoundaryWhite {
				break
			}
		}
	}
	if best == 0 {
		return 0, false
	}
	return a.calculateScore(hay, bestAt, bestAt+len(n)), true
}

// occurrences lists where n appears in h, case as written, without overlaps
// (memchr's memmem::find_iter).
func occurrences(h, n []rune) []int {
	var out []int
	for i := 0; i+len(n) <= len(h); {
		if slices.Equal(h[i:i+len(n)], n) {
			out = append(out, i)
			i += len(n)
			continue
		}
		i++
	}
	return out
}

func (a atom) restMatches(hay text, i int) bool {
	n := a.needle.runes
	end := min(i+len(n), len(hay.runes))
	if end-(i+1) != len(n)-1 {
		return false
	}
	for j, r := range hay.runes[i+1 : end] {
		if a.norm(r, hay.ascii) != n[j+1] {
			return false
		}
	}
	return true
}

// substringUnicode keeps nucleo's quirks: it never tries the last place the
// needle could start, and the class before a rune is the class of the last
// rune that matched the needle's first.
func (a atom) substringUnicode(hay text, start int) (uint16, bool) {
	n, h := a.needle.runes, hay.runes
	prev := prevClass(hay, start)
	var best uint16
	bestAt := 0
	for i := start; i < len(h)-len(n); i++ {
		r, c := a.classAndNorm(h[i], false)
		if r != n[0] {
			continue
		}
		bonus := bonusFor(prev, c)
		prev = c
		score := bonus*bonusFirstCharFactor + scoreMatch
		if score > best && a.restMatches(hay, i) {
			best, bestAt = score, i
			if bonus >= bonusBoundaryWhite {
				break
			}
		}
	}
	if best == 0 {
		return 0, false
	}
	return a.calculateScore(hay, bestAt, bestAt+len(n)), true
}

// cell is nucleo's ScoreCell.
type cell struct {
	score       uint16
	consecutive uint8 // bonus
	matched     bool
}

// unmatched cannot occur from a match: a matched cell's consecutive bonus is
// at least bonusConsecutive.
var unmatched = cell{matched: true}

// optimal is nucleo's fuzzy_match_optimal: the best score over every way to
// place the needle in hay[start:end].
func (a atom) optimal(hay text, start, greedyEnd, end int) (uint16, bool) {
	n := a.needle.runes
	length := end - start
	if !matrixFits(length, len(n), hay.ascii) {
		return a.greedy(hay, start, greedyEnd)
	}
	h := make([]rune, length)
	bonus := make([]uint8, length)
	rows := make([]int, len(n))
	prev := prevClass(hay, start)
	row, matched := 0, false
	for i, r := range hay.runes[start:end] {
		r, c := a.classAndNorm(r, hay.ascii)
		h[i] = r
		bonus[i] = uint8(bonusFor(prev, c))
		prev = c
		if r != n[row] {
			continue
		}
		switch {
		case row < len(n)-1:
			rows[row] = i
			row++
		case !matched:
			rows[row] = i
			matched = true
		}
	}
	if !matched {
		return 0, false
	}
	current := make([]cell, length+1-len(n))
	scoreRow(true, current, h, bonus, 0, rows[1], 0, n[0], n[1])
	for i := 1; i < len(n)-1; i++ {
		scoreRow(false, current, h, bonus, rows[i], rows[i+1], i, n[i], n[i+1])
	}
	var best uint16
	for _, c := range current[rows[len(n)-1]+1-len(n):] {
		best = max(best, c.score)
	}
	return best, true
}

// matrixFits reports whether nucleo's preallocated matrix holds the
// problem; it falls back to the greedy match when it does not.
func matrixFits(length, needle int, ascii bool) bool {
	if length*needle > maxMatrixCells || length > maxMatrixHaystack || needle > maxMatrixNeedle {
		return false
	}
	runeSize := 4
	if ascii {
		runeSize = 1
	}
	width := length + 1 - needle
	size := length*runeSize + length
	size = alignUp(size, 2) + 2*needle
	size = alignUp(size, 8) + 8*width
	size += width * needle
	return size <= matcherDataSize
}

func alignUp(n, to int) int { return (n + to - 1) / to * to }

func scoreRow(first bool, current []cell, h []rune, bonus []uint8, rowOff, nextRowOff, needleIdx int, want, nextWant rune) {
	nextRowOff--
	relRowOff := rowOff - needleIdx
	nextRelRowOff := nextRowOff - needleIdx
	var prevP, prevM uint16
	firstCell := func(j int) cell {
		if h[j] != want {
			return unmatched
		}
		return cell{score: uint16(bonus[j])*bonusFirstCharFactor + scoreMatch, consecutive: bonus[j]}
	}
	for k := range nextRowOff - rowOff {
		p := pScore(prevP, prevM)
		m := current[relRowOff+k]
		if first {
			m = firstCell(rowOff + k)
		}
		prevP, prevM = p, m.score
	}
	for k := 0; nextRowOff+k+1 < len(h) && nextRelRowOff+k < len(current); k++ {
		j := nextRowOff + k
		p := pScore(prevP, prevM)
		m := current[nextRelRowOff+k]
		if first {
			m = firstCell(j)
		}
		if h[j+1] == nextWant {
			current[nextRelRowOff+k] = nextCell(p, uint16(bonus[j+1]), m)
		} else {
			current[nextRelRowOff+k] = unmatched
		}
		prevP, prevM = p, m.score
	}
}

func pScore(prevP, prevM uint16) uint16 {
	match := max(prevM, penaltyGapStart) - penaltyGapStart
	skip := max(prevP, penaltyGapExtension) - penaltyGapExtension
	return max(match, skip)
}

func nextCell(p, bonus uint16, m cell) cell {
	if m == unmatched {
		return cell{score: p + bonus + scoreMatch, consecutive: uint8(bonus)}
	}
	consecutive := max(uint16(m.consecutive), bonusConsecutive)
	if bonus >= bonusBoundary && bonus > consecutive {
		consecutive = bonus
	}
	match := m.score + max(consecutive, bonus)
	skip := p + bonus
	if match > skip {
		return cell{score: match + scoreMatch, consecutive: uint8(consecutive), matched: true}
	}
	return cell{score: skip + scoreMatch, consecutive: uint8(bonus)}
}

// greedy is nucleo's fallback for a haystack too long for the matrix.
func (a atom) greedy(hay text, start, end int) (uint16, bool) {
	n, h := a.needle.runes, hay.runes
	if !hay.ascii || !a.needle.ascii {
		if len(n) > 1 {
			next, found := 1, false
			for i := end; i < len(h); i++ {
				if a.norm(h[i], hay.ascii) != n[next] {
					continue
				}
				next++
				if next == len(n) {
					end, found = i+1, true
					break
				}
			}
			if !found {
				return 0, false
			}
		}
	}
	next := len(n) - 1
	for i := end - 1; i >= start; i-- {
		if a.norm(h[i], hay.ascii) != n[next] {
			continue
		}
		if next == 0 {
			start = i
			break
		}
		next--
	}
	return a.calculateScore(hay, start, end), true
}
