// Package clipcard finds a card for the clipboard text: a color, a Unix
// time, or JSON, decoded, with ways to copy it in other forms. It is a port
// of the Rust app's features/clip_card.rs.
package clipcard

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/color"
	"github.com/jewei/tinydash/internal/feature/unixtime"
)

const (
	// MaxCardBytes is the longest text that gets a card: reading and
	// formatting more would slow the pane down.
	MaxCardBytes = 1_000_000
	// MaxJSONLines is how many lines the full JSON view shows; Copy Pretty
	// JSON has them all.
	MaxJSONLines = 2_000
	// MaxTokenChars is where the JSON view cuts a longer string or number.
	MaxTokenChars = 500
	// MaxJSONKeys is how many top-level keys the card names.
	MaxJSONKeys = 12
	// MaxMinifiedChars is how much of the minified JSON the card shows.
	MaxMinifiedChars = 300
	// MaxPrettyBytes is the longest pretty JSON the card copies. Indents
	// grow with depth, so deep JSON under MaxCardBytes could otherwise
	// take gigabytes; such JSON has no Copy Pretty JSON.
	MaxPrettyBytes = 4 * MaxCardBytes
)

// Card is what the clipboard text decodes to. Exactly one of Color, Time,
// and JSON is set.
type Card struct {
	// ID changes with the text, so a dismissed card comes back only for a
	// new copy.
	ID    string
	Color *color.Color
	Time  *unixtime.Time
	JSON  *JSON
	// Actions copy the content in other forms; the first is the main one.
	// A color's and a time's copy their rows in order.
	Actions []core.ResultAction
}

// JSON is what the card and the full view show of JSON.
type JSON struct {
	// Array is set for an array, clear for an object.
	Array bool
	// Count is the keys of an object, or the items of an array.
	Count int
	// Depth is how deep objects and arrays nest; the outer one is 1.
	Depth int
	// Bytes is the size of the text without the space around it.
	Bytes int
	// Keys are the first top-level keys of an object.
	Keys []Key
	// Minified is the start of the JSON without its spaces.
	Minified string
	// Lines are the first lines of the JSON, laid out.
	Lines []Line
	// MoreLines is how many lines the view leaves out.
	MoreLines int
}

// Key is a top-level key, as written between its quotes, with a hint of its
// value: `{3}` for an object of three keys, `[2]` for an array of two
// items, or "" for any other value.
type Key struct {
	Name, Hint string
}

// Line is one line of the laid out JSON. Indent counts levels of two
// spaces.
type Line struct {
	Indent int
	Tokens []Token
}

// Token is a piece of a line, colored by its kind.
type Token struct {
	Kind TokenKind
	Text string
}

// TokenKind is what a token is in JSON.
type TokenKind uint8

const (
	// TokenKey is an object key, with its quotes.
	TokenKey TokenKind = iota
	TokenString
	TokenNumber
	// TokenLiteral is true, false, or null.
	TokenLiteral
	// TokenPunctuation is a bracket, a brace, a comma, or ": ".
	TokenPunctuation
)

// Detect returns the card for the clipboard text, or false when the text is
// no color, Unix time, or JSON. A Unix time's local time is in now's
// location.
func Detect(text string, now time.Time) (Card, bool) {
	if len(text) > MaxCardBytes {
		return Card{}, false
	}
	card := Card{ID: fmt.Sprintf("%016x", fnv(text))}
	trimmed := strings.TrimSpace(text)
	if c, ok := color.Decode(trimmed); ok {
		card.Color, card.Actions = &c, c.Actions()
		return card, true
	}
	if t, ok := unixtime.Decode(trimmed, now); ok {
		card.Time, card.Actions = &t, t.Actions()
		return card, true
	}
	if !isJSON(trimmed) {
		return Card{}, false
	}
	tokens := tokenize(trimmed)
	var lines []Line
	shape := layout(tokens, true, func(line Line) {
		if len(lines) < MaxJSONLines {
			lines = append(lines, line)
		}
	})
	minified := minify(tokens)
	card.JSON = &JSON{
		Array:     trimmed[0] == '[',
		Count:     shape.count,
		Depth:     shape.depth,
		Bytes:     len(trimmed),
		Keys:      shape.keys,
		Minified:  firstChars(minified, MaxMinifiedChars),
		Lines:     lines,
		MoreLines: shape.lines - len(lines),
	}
	if text, ok := pretty(tokens); ok {
		card.Actions = append(card.Actions, core.Do("Copy Pretty JSON", core.Copy{Text: text}))
	}
	card.Actions = append(card.Actions, core.Do("Copy Minified JSON", core.Copy{Text: minified}))
	return card, true
}

// fnv is the 64-bit FNV-1a hash, as the Rust app made card IDs.
func fnv(text string) uint64 {
	hash := uint64(0xcbf29ce484222325)
	for i := 0; i < len(text); i++ {
		hash = (hash ^ uint64(text[i])) * 0x100000001b3
	}
	return hash
}

// isJSON accepts an object or an array, as the Rust app's serde_json did.
// Text that is not UTF-8 is never JSON there.
func isJSON(text string) bool {
	return text != "" && (text[0] == '{' || text[0] == '[') && utf8.ValidString(text) && json.Valid([]byte(text))
}

func firstChars(text string, n int) string {
	for i := range text {
		if n == 0 {
			return text[:i]
		}
		n--
	}
	return text
}

func minify(tokens []Token) string {
	var b strings.Builder
	for _, t := range tokens {
		b.WriteString(t.Text)
	}
	return b.String()
}

// pretty lays out all of the JSON, two spaces a level, for Copy, or
// false when that takes more than MaxPrettyBytes.
func pretty(tokens []Token) (string, bool) {
	var b strings.Builder
	fits := true
	layout(tokens, false, func(line Line) {
		size := 2 * line.Indent
		for _, t := range line.Tokens {
			size += len(t.Text)
		}
		if b.Len() > 0 {
			size++
		}
		if !fits || b.Len()+size > MaxPrettyBytes {
			fits = false
			return
		}
		if b.Len() > 0 {
			b.WriteByte('\n')
		}
		b.WriteString(strings.Repeat("  ", line.Indent))
		for _, t := range line.Tokens {
			b.WriteString(t.Text)
		}
	})
	if !fits {
		return "", false
	}
	return b.String(), true
}

// tokenize splits valid JSON into tokens and skips the space between them.
// A string before a colon is a key.
func tokenize(text string) []Token {
	var tokens []Token
	at := 0
	for at < len(text) {
		start := at
		var kind TokenKind
		switch text[at] {
		case ' ', '\t', '\n', '\r':
			at++
			continue
		case '"':
			at++
			for at < len(text) && text[at] != '"' {
				if text[at] == '\\' {
					at += 2
				} else {
					at++
				}
			}
			at = min(at+1, len(text))
			kind = TokenString
		case '{', '}', '[', ']', ':', ',':
			at++
			kind = TokenPunctuation
		case 't', 'f', 'n':
			for at < len(text) && ('a' <= text[at] && text[at] <= 'z' || 'A' <= text[at] && text[at] <= 'Z') {
				at++
			}
			kind = TokenLiteral
		default:
			for at < len(text) && strings.IndexByte("0123456789-+.eE", text[at]) >= 0 {
				at++
			}
			kind = TokenNumber
		}
		tokens = append(tokens, Token{Kind: kind, Text: text[start:at]})
	}
	for i := 1; i < len(tokens); i++ {
		if tokens[i].Text == ":" && tokens[i-1].Kind == TokenString {
			tokens[i-1].Kind = TokenKey
		}
	}
	return tokens
}

// shape is what the card says about the JSON besides its lines.
type shape struct {
	count, depth int
	keys         []Key
	// lines is how many lines all of the JSON takes.
	lines int
}

// container is an object or an array that is open while laying out.
type container struct {
	array bool
	// children are the keys of an object, or the items of an array, so far.
	children int
}

// layout lays the tokens out in lines, indented a level for each open
// object or array, and hands each line to emit. An empty object or array
// stays on one line. With cutLong, a token over MaxTokenChars is cut.
func layout(tokens []Token, cutLong bool, emit func(Line)) shape {
	var line Line
	var open []container
	var s shape
	// topKey is the index in s.keys of the top-level key whose value is
	// being laid out, or -1.
	topKey := -1

	endLine := func(indent int) {
		if len(line.Tokens) > 0 {
			s.lines++
			emit(line)
		}
		line = Line{Indent: indent}
	}
	push := func(kind TokenKind, text string) {
		if cutLong && utf8.RuneCountInString(text) > MaxTokenChars {
			text = firstChars(text, MaxTokenChars) + "…"
		}
		line.Tokens = append(line.Tokens, Token{Kind: kind, Text: text})
	}
	// A value starts: an array counts it; an object counted its key.
	countValue := func() {
		if n := len(open); n > 0 && open[n-1].array {
			open[n-1].children++
		}
	}

	for i, t := range tokens {
		switch t.Text {
		case "{", "[":
			countValue()
			push(TokenPunctuation, t.Text)
			open = append(open, container{array: t.Text == "["})
			s.depth = max(s.depth, len(open))
			closing := "}"
			if t.Text == "[" {
				closing = "]"
			}
			// {} and [] stay on one line.
			if i+1 >= len(tokens) || tokens[i+1].Text != closing {
				endLine(len(open))
			}
		case "}", "]":
			closed := 0
			if n := len(open); n > 0 {
				closed = open[n-1].children
				open = open[:n-1]
			}
			if closed > 0 {
				endLine(len(open))
			}
			push(TokenPunctuation, t.Text)
			switch len(open) {
			case 0:
				s.count = closed
			case 1:
				if topKey >= 0 {
					if t.Text == "}" {
						s.keys[topKey].Hint = fmt.Sprintf("{%d}", closed)
					} else {
						s.keys[topKey].Hint = fmt.Sprintf("[%d]", closed)
					}
					topKey = -1
				}
			}
		case ",":
			push(TokenPunctuation, ",")
			endLine(len(open))
		case ":":
			push(TokenPunctuation, ": ")
		default:
			if t.Kind == TokenKey {
				if n := len(open); n > 0 {
					open[n-1].children++
				}
				if len(open) == 1 {
					topKey = -1
					if len(s.keys) < MaxJSONKeys {
						// Without its quotes.
						s.keys = append(s.keys, Key{Name: t.Text[1 : len(t.Text)-1]})
						topKey = len(s.keys) - 1
					}
				}
			} else {
				countValue()
			}
			push(t.Kind, t.Text)
		}
	}
	endLine(0)
	return s
}
