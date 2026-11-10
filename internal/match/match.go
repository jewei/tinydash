// Package match scores names, aliases, and long text against a query, as the
// Rust app's search/matcher.rs did. The fuzzy tier is a port of the parts of
// nucleo-matcher 0.3.1 that TinyDash calls, so scores and ranking match the
// Rust app's.
package match

import (
	"strings"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/unicode/norm"
)

const (
	// Exact is the bonus for a name that equals the query, ignoring case.
	Exact uint32 = 10_000
	// ExactAlias: scores at or above it come only from an alias that equals
	// the query.
	ExactAlias = 2 * Exact
	// Prefix is the bonus for a name that starts with the query.
	Prefix uint32 = 3_000
	// WordPrefix is the bonus for a name with a word that starts with the query.
	WordPrefix uint32 = 500
	// KeywordPenalty: a localized keyword ranks below an equally good name match.
	KeywordPenalty uint32 = 300
	// Strong: scores at or above it come from a name, alias, or keyword that
	// starts with the query (2,700 or more). Any other match, with the most
	// usage (1,000), stays below: 499 + 500 + 1,000 = 1,999.
	Strong = Prefix - KeywordPenalty

	// An alias ranks a little below the same match on the name.
	aliasPenalty uint32 = 100
	// Fuzzy scores grow with the query's length; squeeze keeps them below
	// this, so they never reach the word-prefix bonus.
	maxFuzzy = WordPrefix - 1
)

// Matcher scores candidates against one query.
type Matcher struct {
	query string
	fuzzy []atom
	words []atom
	// buf holds the runes of the ASCII text being scored, reused so that
	// a search over 55,000 names allocates no runes for them. A Matcher is
	// therefore not safe for use by more than one goroutine at a time.
	buf []rune
}

// New makes a matcher for a query.
func New(query string) *Matcher {
	query = Normalize(query)
	m := &Matcher{query: query}
	// One atom per word, with `\` kept as typed: nucleo's own pattern parser
	// reads `\ ` as an escaped space, so Windows paths would not match.
	for word := range strings.SplitSeq(query, " ") {
		if word != "" {
			m.fuzzy = append(m.fuzzy, newAtom(word, false))
			m.words = append(m.words, newAtom(word, true))
		}
	}
	return m
}

// Query is the query in NFC lowercase, with single spaces.
func (m *Matcher) Query() string { return m.query }

// Name scores a name; ok is false when it does not match.
func (m *Matcher) Name(name string) (score uint32, ok bool) {
	fuzzy, ok := m.scoreAtoms(m.fuzzy, name)
	if !ok {
		return 0, false
	}
	switch prefix, equal := foldMatch(name, m.query); {
	case equal:
		return fuzzy + Exact, true
	case prefix:
		return fuzzy + Prefix, true
	}
	if hasWordPrefix(name, m.query) {
		return fuzzy + WordPrefix, true
	}
	return fuzzy, true
}

// hasWordPrefix reports whether a word of name (a run of alphanumeric
// runes) starts with the query.
func hasWordPrefix(name, query string) bool {
	start := -1
	for i, r := range name {
		switch {
		case isAlphanumeric(r):
			if start < 0 {
				start = i
			}
		case start >= 0:
			if hasPrefixFold(name[start:i], query) {
				return true
			}
			start = -1
		}
	}
	return start >= 0 && hasPrefixFold(name[start:], query)
}

// Words scores text in which every query word appears as written, for long
// text such as paths, where fuzzy matching finds too much.
func (m *Matcher) Words(text string) (uint32, bool) { return m.scoreAtoms(m.words, text) }

// Alias scores an alias the user chose. Only an alias that starts with the
// query matches, and one that equals it ranks above any name match.
func (m *Matcher) Alias(alias string) (uint32, bool) {
	score, ok := m.Name(alias)
	switch {
	case !ok:
		return 0, false
	case equalFold(alias, m.query):
		return score + Exact, true
	case hasPrefixFold(alias, m.query):
		return score, true
	}
	return 0, false
}

// Best is the best name score among a name and its aliases.
func (m *Matcher) Best(name string, aliases ...string) (uint32, bool) {
	best, found := m.Name(name)
	for _, alias := range aliases {
		score, ok := m.Name(alias)
		if !ok {
			continue
		}
		score = max(score, aliasPenalty) - aliasPenalty
		if !found || score > best {
			best, found = score, true
		}
	}
	return best, found
}

func (m *Matcher) scoreAtoms(atoms []atom, text string) (uint32, bool) {
	if cannotMatchASCII(atoms, text) {
		return 0, false
	}
	hay := newText(text, &m.buf)
	var total uint32
	for _, a := range atoms {
		s, ok := a.score(hay)
		if !ok {
			return 0, false
		}
		total += uint32(s)
	}
	return squeeze(total), true
}

// cannotMatchASCII rejects most ASCII text before it is copied for
// scoring, in one pass. Every way nucleo matches an atom in ASCII text
// finds the atom's runes in order, case ignored, and an atom that is not
// ASCII never matches ASCII text. It reports true only when text is ASCII:
// an atom it rejects was scanned to the end of text without a byte of
// 0x80 or above, and a byte like that stops the scan with false.
func cannotMatchASCII(atoms []atom, text string) bool {
	for _, a := range atoms {
		if !a.needle.ascii {
			return isASCII(text)
		}
		i := 0
		for _, r := range a.needle.runes {
			lower, upper := byte(r), byte(r)
			if 'a' <= r && r <= 'z' {
				upper = byte(r - asciiLetterCaseOffset)
			}
			for ; i < len(text); i++ {
				b := text[i]
				if b >= 0x80 {
					return false
				}
				if b == lower || b == upper {
					break
				}
			}
			if i == len(text) {
				return true
			}
			i++
		}
	}
	return false
}

// squeeze maps a fuzzy score below maxFuzzy. A higher score never maps
// lower, so long matches still rank by quality, though close high scores may
// tie.
func squeeze(score uint32) uint32 {
	return uint32(uint64(maxFuzzy) * uint64(score) / (uint64(score) + 500))
}

// Normalize returns text in NFC, lowercase, with single spaces: the form all
// matching compares.
func Normalize(text string) string {
	joined := strings.Join(strings.Fields(norm.NFC.String(text)), " ")
	runes := []rune(joined)
	var b strings.Builder
	for i, r := range runes {
		if r == 'Σ' && finalSigma(runes, i) {
			b.WriteRune('ς')
			continue
		}
		b.WriteString(lower(r))
	}
	return b.String()
}

// lower is Unicode's full lowercase mapping of one rune. Only İ maps to more
// than one rune without context.
func lower(r rune) string {
	if r == 'İ' {
		return "i̇"
	}
	return string(unicode.ToLower(r))
}

// finalSigma reports whether the Σ at i ends a word, where Rust's
// str::to_lowercase writes ς.
func finalSigma(runes []rune, i int) bool {
	before := false
	for j := i - 1; j >= 0; j-- {
		if !caseIgnorable(runes[j]) {
			before = cased(runes[j])
			break
		}
	}
	for _, r := range runes[i+1:] {
		if !caseIgnorable(r) {
			return before && !cased(r)
		}
	}
	return before
}

func cased(r rune) bool {
	return unicode.In(r, unicode.Lu, unicode.Ll, unicode.Lt, unicode.Other_Lowercase, unicode.Other_Uppercase)
}

func caseIgnorable(r rune) bool {
	return unicode.In(r, unicode.Mn, unicode.Me, unicode.Cf, unicode.Lm, unicode.Sk) ||
		strings.ContainsRune("'.:··՟״‘’․‧︓﹒﹕＇．：", r)
}

// foldMatch walks text in lowercase, rune by rune without NFC, against a
// query that Normalize made. It reports whether the query starts the
// lowercase text and whether it is all of it. It does not allocate: it runs
// for every name of every search.
func foldMatch(text, query string) (prefix, equal bool) {
	rest := query
	for _, r := range text {
		low, n := [2]rune{unicode.ToLower(r)}, 1
		if r == 'İ' {
			low, n = [2]rune{'i', '\u0307'}, 2
		}
		for _, l := range low[:n] {
			if rest == "" {
				return true, false
			}
			want, size := utf8.DecodeRuneInString(rest)
			if want != l {
				return false, false
			}
			rest = rest[size:]
		}
	}
	return rest == "", rest == ""
}

func hasPrefixFold(text, prefix string) bool {
	ok, _ := foldMatch(text, prefix)
	return ok
}

func equalFold(text, query string) bool {
	_, ok := foldMatch(text, query)
	return ok
}

func isAlphanumeric(r rune) bool {
	return unicode.IsLetter(r) || unicode.IsNumber(r) || unicode.Is(unicode.Other_Alphabetic, r)
}
