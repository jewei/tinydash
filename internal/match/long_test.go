package match

import (
	"strings"
	"testing"
)

// filler is long enough that a match across it does not fit nucleo's
// matrix, so these tests reach its greedy fallback.
var filler = strings.Repeat("x", 20_000)

func fuzzyScore(query, hay string) (uint32, bool) {
	m := New(query)
	return m.scoreAtoms(m.fuzzy, hay)
}

func TestTheLongTextReallyLeavesTheMatrix(t *testing.T) {
	for _, ascii := range []bool{true, false} {
		if matrixFits(len(filler), 3, ascii) {
			t.Fatalf("a %d-character match fits the matrix (ascii %v): the tests below miss greedy", len(filler), ascii)
		}
	}
}

// Long clipboard entries and paths match by the same rule as short ones:
// every letter of the word, in order, in any case. A word without marks
// matches letters with them; a word with a mark asks for it.
func TestLongTextMatchesTheWordsLettersInOrder(t *testing.T) {
	for _, c := range []struct {
		query, hay string
		want       bool
	}{
		{"rep", "R" + filler + "e" + filler + "P", true},
		{"rep", "p" + filler + "e" + filler + "r", false},
		{"rep", "r" + filler + "e", false},
		{"cafe", "Ça" + filler + "fé ✓", true},
		{"café", "ca" + filler + "fé ✓", true},
		{"café", "ca" + filler + "fe ✓", false},
		{"über", "ü" + filler + "ber ✓", true},
		{"uber", "ü" + filler + "ber ✓", true},
		{"ber ü", "ü" + filler + "ber ✓", true},
		{"rüb", "r" + filler + "ü", false},
	} {
		if _, ok := fuzzyScore(c.query, c.hay); ok != c.want {
			parts := strings.Split(c.hay, filler)
			t.Errorf("%q in %q, filler, %q: matched %v, want %v", c.query, parts[0], parts[len(parts)-1], ok, c.want)
		}
	}
}

// The fallback places the word as close to its end as it can. A word held
// together after a long run of text scores as it does in short text, and
// above the same word spread out.
func TestLongTextRanksAWordTogetherAboveItSpreadOut(t *testing.T) {
	for _, c := range []struct{ query, together, short, spread string }{
		{"rep", "r" + filler + " report", "x report", "r" + filler + "e" + filler + "p"},
		{"café", "c" + filler + " café ✓", "x café ✓", "c" + filler + "a" + filler + "fé ✓"},
	} {
		together, ok := fuzzyScore(c.query, c.together)
		short, ok2 := fuzzyScore(c.query, c.short)
		spread, ok3 := fuzzyScore(c.query, c.spread)
		if !ok || !ok2 || !ok3 || together != short || together <= spread {
			t.Errorf("%q: together %d, in short text %d, spread %d", c.query, together, short, spread)
		}
	}
}
