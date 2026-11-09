package match

import (
	"slices"
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
