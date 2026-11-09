package match

import (
	"strings"
	"sync"
	"unicode"
)

// text is a string as nucleo-matcher sees it: one rune per grapheme, its
// first. ascii marks nucleo's byte form, which it uses when every grapheme
// starts with an ASCII rune; it then keeps every byte of the string, so the
// bytes of a combining mark take part as non-word characters.
type text struct {
	runes []rune
	ascii bool
}

func newText(s string) text {
	if !isASCII(s) {
		starts := graphemeStarts(s)
		for _, r := range starts {
			if r >= 0x80 {
				return text{starts, false}
			}
		}
	}
	bytes := make([]rune, len(s))
	for i := range len(s) {
		bytes[i] = rune(s[i])
	}
	return text{bytes, true}
}

type charClass uint8

const (
	classWhitespace charClass = iota
	classNonWord
	classDelimiter
	classLower
	classUpper
	classLetter
	classNumber
)

// nucleo's scores, its default configuration, and the limits of its matrix.
const (
	scoreMatch            = 16
	penaltyGapStart       = 3
	penaltyGapExtension   = 1
	bonusBoundary         = scoreMatch / 2
	bonusBoundaryWhite    = bonusBoundary + 2
	bonusBoundaryDelimit  = bonusBoundary + 1
	bonusCamel123         = bonusBoundary - penaltyGapStart
	bonusNonWord          = bonusBoundary
	bonusConsecutive      = penaltyGapStart + penaltyGapExtension
	bonusFirstCharFactor  = 2
	initialClass          = classWhitespace
	delimiters            = "/,:;|"
	maxMatrixCells        = 100 * 1024
	maxMatrixHaystack     = 1<<16 - 1
	maxMatrixNeedle       = 2048
	matcherDataSize       = 2048*4 + 2048 + 2048*2 + 2048*8 + maxMatrixCells
	asciiLetterCaseOffset = 'a' - 'A'
)

// class is a rune's class. In the byte form every rune is a byte, and bytes
// from 0x80 are non-word characters.
func class(r rune, ascii bool) charClass {
	if ascii || r < 0x80 {
		switch {
		case 'a' <= r && r <= 'z':
			return classLower
		case 'A' <= r && r <= 'Z':
			return classUpper
		case '0' <= r && r <= '9':
			return classNumber
		case r == ' ' || r == '\t' || r == '\n' || r == '\f' || r == '\r':
			return classWhitespace
		case strings.ContainsRune(delimiters, r):
			return classDelimiter
		}
		return classNonWord
	}
	switch {
	case unicode.In(r, unicode.Ll, unicode.Other_Lowercase):
		return classLower
	case fold(r) != r:
		return classUpper
	case unicode.IsNumber(r):
		return classNumber
	case unicode.IsLetter(r) || unicode.Is(unicode.Other_Alphabetic, r):
		return classLetter
	case unicode.Is(unicode.White_Space, r):
		return classWhitespace
	}
	return classNonWord
}

// classAndNormalize is nucleo's char_class_and_normalize, case ignored. It
// folds a lowercase rune only when normalizing, as nucleo does.
func classAndNormalize(r rune, ascii, normalize bool) (rune, charClass) {
	c := class(r, ascii)
	if ascii || r < 0x80 {
		if c == classUpper {
			r += asciiLetterCaseOffset
		}
		return r, c
	}
	caseFold := c == classUpper
	if normalize {
		r = latin(r)
		caseFold = true
	}
	if caseFold {
		r = fold(r)
	}
	return r, c
}

// normalizeRune is nucleo's Char::normalize, case ignored.
func normalizeRune(r rune, ascii, normalize bool) rune {
	if ascii || r < 0x80 {
		if 'A' <= r && r <= 'Z' {
			r += asciiLetterCaseOffset
		}
		return r
	}
	if normalize {
		r = latin(r)
	}
	return fold(r)
}

func bonusFor(prev, c charClass) uint16 {
	if c > classDelimiter {
		switch prev {
		case classWhitespace:
			return bonusBoundaryWhite
		case classDelimiter:
			return bonusBoundaryDelimit
		case classNonWord:
			return bonusBoundary
		}
	}
	switch {
	case prev == classLower && c == classUpper, prev != classNumber && c == classNumber:
		return bonusCamel123
	case c == classWhitespace:
		return bonusBoundaryWhite
	case c == classNonWord:
		return bonusNonWord
	}
	return 0
}

// fold is Unicode simple case folding, which nucleo uses to ignore case.
// Upper-then-lower gives it for every rune of Unicode 15, nucleo's table,
// except the ones handled here. Runes added since then fold here but not in
// nucleo; no name or keyword TinyDash ships uses them.
func fold(r rune) rune {
	switch {
	case 0x13A0 <= r && r <= 0x13F5: // Cherokee folds to its capitals.
		return r
	case 0x13F8 <= r && r <= 0x13FD:
		return r - 8
	case 0xAB70 <= r && r <= 0xABBF:
		return r - 0xAB70 + 0x13A0
	case r == 'İ' || r == 'ı': // Only Turkic folding maps these.
		return r
	}
	return unicode.ToLower(unicode.ToUpper(r))
}

func latin(r rune) rune {
	if to, ok := latinTable[r]; ok {
		return to
	}
	return r
}

// graphemeStarts returns the first rune of each extended grapheme cluster.
// It follows UAX #29 except for Prepend and Indic conjuncts, which no
// TinyDash name or keyword needs, and takes Extended_Pictographic to be the
// symbols (So) and the emoji planes.
func graphemeStarts(s string) []rune {
	var starts []rune
	prev := propNone
	regional := 0         // regional indicators in a row
	pictographic := false // an Extended_Pictographic, then Extend runes
	for _, r := range s {
		p := propOf(r)
		if graphemeBreak(prev, p, regional, pictographic) {
			starts = append(starts, r)
		}
		if p == propRegional {
			regional++
		} else {
			regional = 0
		}
		switch p {
		case propPictographic:
			pictographic = true
		case propExtend, propZWJ:
		default:
			pictographic = false
		}
		prev = p
	}
	return starts
}

// graphemeProp is a rune's Grapheme_Cluster_Break property, with
// Extended_Pictographic.
type graphemeProp uint8

const (
	propNone graphemeProp = iota // before the first rune
	propOther
	propCR
	propLF
	propControl
	propExtend
	propZWJ
	propSpacingMark
	propRegional
	propPictographic
	propL
	propV
	propT
	propLV
	propLVT
)

// bmpProps holds the property of every rune below U+10000: looking each up
// in Unicode's tables for every name of every search is too slow.
var bmpProps = sync.OnceValue(func() *[0x10000]graphemeProp {
	var props [0x10000]graphemeProp
	for r := range rune(len(props)) {
		props[r] = graphemeProperty(r)
	}
	return &props
})

func propOf(r rune) graphemeProp {
	if r < 0x10000 {
		return bmpProps()[r]
	}
	return graphemeProperty(r)
}

func graphemeProperty(r rune) graphemeProp {
	switch {
	case r == '\r':
		return propCR
	case r == '\n':
		return propLF
	case r == '\u200D':
		return propZWJ
	case unicode.In(r, unicode.Mn, unicode.Me, unicode.Other_Grapheme_Extend) || 0x1F3FB <= r && r <= 0x1F3FF:
		return propExtend
	case r == '\u200C', unicode.Is(unicode.Prepended_Concatenation_Mark, r):
		return propOther
	case unicode.In(r, unicode.Cc, unicode.Zl, unicode.Zp, unicode.Cf):
		return propControl
	case unicode.Is(unicode.Mc, r):
		return propSpacingMark
	case 0x1F1E6 <= r && r <= 0x1F1FF:
		return propRegional
	case unicode.Is(unicode.So, r) || 0x1F000 <= r && r <= 0x1FFFD:
		return propPictographic
	case 0x1100 <= r && r <= 0x115F, 0xA960 <= r && r <= 0xA97C:
		return propL
	case 0x1160 <= r && r <= 0x11A7, 0xD7B0 <= r && r <= 0xD7C6:
		return propV
	case 0x11A8 <= r && r <= 0x11FF, 0xD7CB <= r && r <= 0xD7FB:
		return propT
	case 0xAC00 <= r && r <= 0xD7A3 && (r-0xAC00)%28 == 0:
		return propLV
	case 0xAC00 <= r && r <= 0xD7A3:
		return propLVT
	}
	return propOther
}

func graphemeBreak(prev, p graphemeProp, regional int, pictographic bool) bool {
	switch {
	case prev == propNone:
		return true
	case prev == propCR && p == propLF:
		return false
	case isControl(prev) || isControl(p):
		return true
	case prev == propL && (p == propL || p == propV || p == propLV || p == propLVT),
		(prev == propLV || prev == propV) && (p == propV || p == propT),
		(prev == propLVT || prev == propT) && p == propT:
		return false
	case p == propExtend || p == propZWJ || p == propSpacingMark:
		return false
	case prev == propZWJ && pictographic && p == propPictographic:
		return false
	case prev == propRegional && p == propRegional:
		return regional%2 == 0
	}
	return true
}

func isControl(p graphemeProp) bool { return p == propCR || p == propLF || p == propControl }
