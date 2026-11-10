package match

import (
	"cmp"
	"math/rand/v2"
	"slices"
	"strings"
	"testing"
)

// scored returns a score's value and fails the test when there is none.
func scored(t *testing.T) func(uint32, bool) uint32 {
	return func(score uint32, ok bool) uint32 {
		t.Helper()
		if !ok {
			t.Fatal("no match")
		}
		return score
	}
}

func TestExactBeatsPrefixBeatsWordPrefixBeatsFuzzy(t *testing.T) {
	must := scored(t)
	m := New("code")
	exact := must(m.Name("Code"))
	prefix := must(m.Name("Code Editor"))
	word := must(m.Name("Visual Studio Code"))
	fuzzy := must(m.Name("Color Developer"))
	if !(exact > prefix && prefix > word && word > fuzzy) {
		t.Errorf("exact %d, prefix %d, word %d, fuzzy %d", exact, prefix, word, fuzzy)
	}
	if prefix < Strong || word >= Strong {
		t.Errorf("prefix %d and word %d around Strong %d", prefix, word, Strong)
	}
	if _, ok := m.Name("Safari"); ok {
		t.Error("Safari matches code")
	}

	// A long query scores high on fuzzy alone; with the most usage, a
	// word-prefix match must still stay below a prefix match.
	m = New("microsoft visual studio code insiders edition")
	word = must(m.Name("The Microsoft Visual Studio Code Insiders Edition"))
	if word+1_000 >= Strong {
		t.Errorf("word %d", word)
	}
	path := "/Users/me/microsoft visual studio code insiders edition/notes.txt"
	if score := must(m.Words(path)); score >= WordPrefix {
		t.Errorf("words %d", score)
	}
}

// Short queries score little on fuzzy, so a penalty must not push a prefix
// match out of the strong tier, where usage could lift it back.
func TestAShortAliasOrKeywordPrefixIsStrong(t *testing.T) {
	must := scored(t)
	m := New("r")
	if score := must(m.Best("Restart")); score < Strong {
		t.Errorf("name %d", score)
	}
	if score := must(m.Best("Shut Down", "reboot")); score < Strong {
		t.Errorf("alias %d", score)
	}
	if keyword := must(m.Name("reboot")) - KeywordPenalty; keyword < Strong {
		t.Errorf("keyword %d", keyword)
	}
}

func TestLongMatchesKeepTheirOrder(t *testing.T) {
	var scores []uint32
	for _, s := range []uint32{36, 62, 514, 537, 5_000} {
		scores = append(scores, squeeze(s))
	}
	for i := 1; i < len(scores); i++ {
		if scores[i-1] >= scores[i] {
			t.Errorf("%v", scores)
		}
	}
	if squeeze(^uint32(0)) >= WordPrefix {
		t.Error("the largest score reaches the word-prefix bonus")
	}
}

func TestMatchingIgnoresCaseAccentsAndSpacing(t *testing.T) {
	m := New("  CAFE   menu ")
	if m.Query() != "cafe menu" {
		t.Errorf("query %q", m.Query())
	}
	if _, ok := m.Name("Café Menu"); !ok {
		t.Error("Café Menu does not match")
	}
	// An accented query matches only the accented name. Indexes store
	// names in NFC, so decomposed file names compare equal.
	if got := Normalize("Cafe\u0301"); got != "café" {
		t.Errorf("Normalize %q", got)
	}
	if _, ok := New("Cafe\u0301").Name("café"); !ok {
		t.Error("decomposed query does not match")
	}
	if _, ok := New("café").Name("cafe"); ok {
		t.Error("café matches cafe")
	}
}

func TestNormalizeLowercasesAsRustDoes(t *testing.T) {
	for in, want := range map[string]string{
		"ΟΔΟΣ ΣΟΦΊΑ":  "οδος σοφία", // final sigma
		"İstanbul":    "i\u0307stanbul",
		"a\tb\u00A0c": "a b c",
	} {
		if got := Normalize(in); got != want {
			t.Errorf("Normalize(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestAliasesRankJustBelowNames(t *testing.T) {
	must := scored(t)
	m := New("vsc")
	alias := must(m.Best("Visual Studio Code", "vsc"))
	if name := must(m.Name("vsc")); alias != name-100 {
		t.Errorf("alias %d, name %d", alias, name)
	}
}

func TestAnExactUserAliasBeatsAnExactName(t *testing.T) {
	must := scored(t)
	m := New("co")
	if alias := must(m.Alias("co")); alias < ExactAlias {
		t.Errorf("alias %d", alias)
	}
	if name := must(m.Name("Co")); name+1_000 >= ExactAlias {
		t.Errorf("name %d", name)
	}
	if prefix := must(m.Alias("code")); prefix < Strong || prefix+1_000 >= ExactAlias {
		t.Errorf("prefix alias %d", prefix)
	}
	for _, alias := range []string{"deco", "c"} {
		if _, ok := m.Alias(alias); ok {
			t.Errorf("alias %q matches", alias)
		}
	}
}

func TestWordsMustAppearAsWritten(t *testing.T) {
	m := New("project readme")
	if _, ok := m.Words("/Users/me/Project/README.md"); !ok {
		t.Error("the path does not match")
	}
	if _, ok := m.Words("/p/r/o/j/e/c/t/readme"); ok {
		t.Error("scattered letters match")
	}
}

func items(hits []Hit[int]) []int {
	var out []int
	for _, h := range hits {
		out = append(out, h.Item)
	}
	return out
}

func TestTopKeepsTheBestHitsInOrder(t *testing.T) {
	hits := []Hit[int]{{1, 'a'}, {5, 'b'}, {3, 'c'}, {4, 'd'}}
	if got := items(Top(hits, 2)); !slices.Equal(got, []int{'b', 'd'}) {
		t.Errorf("%v", got)
	}
	if got := items(Top([]Hit[int]{{1, 'a'}}, 5)); !slices.Equal(got, []int{'a'}) {
		t.Errorf("%v", got)
	}
}

// Equal scores keep the order they came in, also across the cut.
func TestTopKeepsTiesInInputOrder(t *testing.T) {
	hits := []Hit[int]{{2, 'a'}, {5, 'b'}, {2, 'c'}, {5, 'd'}, {2, 'e'}}
	if got := items(Top(hits, 4)); !slices.Equal(got, []int{'b', 'd', 'a', 'c'}) {
		t.Errorf("%c", got)
	}
}

func TestFoldMatchComparesTheLowercaseText(t *testing.T) {
	for _, c := range []struct {
		text, query   string
		prefix, equal bool
	}{
		{"Code", "code", true, true},
		{"Code", "co", true, false},
		{"Co", "code", false, false},
		{"Code", "", true, false},
		{"", "", true, true},
		{"İstanbul", "i", true, false}, // İ lowercases to two runes
		{"İ", "i̇", true, true},
		{"İs", "i̇s", true, true},
		{"ΣΟΦΊΑ", "σοφία", true, true},
		{"a\xffb", "a�b", true, true},
	} {
		prefix, equal := foldMatch(c.text, c.query)
		if prefix != c.prefix || equal != c.equal {
			t.Errorf("foldMatch(%q, %q) = %v, %v, want %v, %v", c.text, c.query, prefix, equal, c.prefix, c.equal)
		}
	}
}

func TestTheASCIIPrefilterKeepsEveryMatch(t *testing.T) {
	candidates := []string{"Report Budget.txt", "home/rep 1/bud 2.txt", "s", "S", "Safari", "x12*8", "café", "zz", "áb"}
	for _, query := range []string{"s", "sa", "rep bud", "12 * 8", "zzzz", "ab", "café", "REP"} {
		m := New(query)
		for _, c := range candidates {
			_, fuzzy := m.scoreAtoms(m.fuzzy, c)
			_, words := m.scoreAtoms(m.words, c)
			hay := newText(c, new([]rune))
			wantFuzzy, wantWords := true, true
			for _, a := range m.fuzzy {
				_, ok := a.score(hay)
				wantFuzzy = wantFuzzy && ok
			}
			for _, a := range m.words {
				_, ok := a.score(hay)
				wantWords = wantWords && ok
			}
			if fuzzy != wantFuzzy || words != wantWords {
				t.Errorf("%q in %q: fuzzy %v, words %v; want %v, %v", query, c, fuzzy, words, wantFuzzy, wantWords)
			}
		}
	}
}

// Top is the first limit hits of a stable sort, ties in input order, for
// every size and limit, with many equal scores.
func TestTopIsTheStartOfAStableSort(t *testing.T) {
	r := rand.New(rand.NewPCG(3, 4))
	for range 2000 {
		n := r.IntN(60)
		hits := make([]Hit[int], n)
		for i := range hits {
			hits[i] = Hit[int]{Score: uint32(r.IntN(6)), Item: i}
		}
		limit := r.IntN(70)
		want := slices.Clone(hits)
		slices.SortStableFunc(want, func(a, b Hit[int]) int { return cmp.Compare(b.Score, a.Score) })
		want = want[:min(limit, n)]
		if got := Top(slices.Clone(hits), limit); !slices.Equal(got, want) {
			t.Fatalf("n %d limit %d:\ngot  %v\nwant %v", n, limit, got, want)
		}
	}
}

// cannotMatchASCII rejects exactly what the two-pass check it replaced
// rejected: ASCII text in which some atom's runes do not appear in order.
func TestCannotMatchASCIIIsTheTwoPassCheck(t *testing.T) {
	twoPass := func(atoms []atom, text string) bool {
		if !isASCII(text) {
			return false
		}
		for _, a := range atoms {
			if !a.needle.ascii {
				return true
			}
			i := 0
			for _, r := range a.needle.runes {
				for i < len(text) && !equalFoldASCII(rune(text[i]), r) {
					i++
				}
				if i == len(text) {
					return true
				}
				i++
			}
		}
		return false
	}
	alphabet := []string{"a", "b", "s", "A", "B", "S", " ", "-", "é", "ß", "1"}
	r := rand.New(rand.NewPCG(5, 6))
	word := func(n int) string {
		var b strings.Builder
		for range n {
			b.WriteString(alphabet[r.IntN(len(alphabet))])
		}
		return b.String()
	}
	for range 20_000 {
		m := New(word(1+r.IntN(3)) + " " + word(r.IntN(3)))
		text := word(r.IntN(12))
		for _, atoms := range [][]atom{m.fuzzy, m.words} {
			if got, want := cannotMatchASCII(atoms, text), twoPass(atoms, text); got != want {
				t.Fatalf("query %q text %q: got %v, want %v", m.query, text, got, want)
			}
		}
	}
}
