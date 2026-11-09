// Package library holds the snippets (text templates) and quicklinks (URL or
// path templates) that the user saves in Settings, and the `?` list of
// keywords.
//
// Both take the placeholders {query}, filled from text typed after the
// keyword (`jira ABC-12`), {clipboard}, {date}, {time}, and {datetime}.
package library

import (
	"errors"
	"fmt"
	"strconv"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"golang.org/x/text/unicode/norm"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

// Kind says whether an item is a snippet or a quicklink. The values are the
// ones tinydash.db stores.
type Kind string

const (
	KindSnippet   Kind = "snippet"
	KindQuicklink Kind = "quicklink"
)

// Item is one saved snippet or quicklink.
type Item struct {
	// ID is 0 for an item that is not saved yet; saved IDs start at 1.
	ID   int64
	Kind Kind
	Name string
	// Keyword is an optional short word that finds the item; quicklinks
	// read the text after it.
	Keyword string
	// Text is the snippet body, or the quicklink URL or path template.
	Text string
}

const (
	// MaxItems is the most snippets and quicklinks together.
	MaxItems   = 500
	maxName    = 100
	maxKeyword = 32
	maxText    = 32 * 1024
)

// Validated trims the fields and refuses an item that could not work. The
// name and keyword are kept in NFC, the form the matcher gives the query,
// and the keyword in lowercase. now and home are what a quicklink's
// template is tried with.
func (it Item) Validated(now time.Time, home string) (Item, error) {
	it.Name = norm.NFC.String(strings.TrimSpace(it.Name))
	it.Keyword = match.Normalize(strings.TrimSpace(it.Keyword))
	if it.Kind == KindQuicklink {
		it.Text = strings.TrimSpace(it.Text)
	}
	if it.Name == "" || utf8.RuneCountInString(it.Name) > maxName {
		return Item{}, fmt.Errorf("Enter a name of up to %d characters.", maxName)
	}
	if utf8.RuneCountInString(it.Keyword) > maxKeyword || strings.ContainsFunc(it.Keyword, unicode.IsSpace) {
		return Item{}, fmt.Errorf("A keyword is one word of up to %d characters.", maxKeyword)
	}
	if strings.TrimSpace(it.Text) == "" || len(it.Text) > maxText {
		return Item{}, errors.New("Enter the text, up to 32 KB.")
	}
	if it.Kind == KindQuicklink {
		// Some templates work only with certain values, such as
		// `https://{query}` (needs text) or `http://localhost:{query}`
		// (needs a number or nothing), so a template passes if any sample
		// works. Unsafe schemes and roots fail for every value, and running
		// the quicklink checks the real values again.
		var first error
		for _, sample := range []string{"", "x", "1"} {
			_, err := QuicklinkTarget(it.Text, sample, now, home, func() string { return sample })
			if err == nil {
				return it, nil
			}
			if first == nil {
				first = err
			}
		}
		return Item{}, first
	}
	return it, nil
}

// TakesQuery reports whether text typed after its keyword fills it: a
// quicklink, or a snippet with {query}.
func (it Item) TakesQuery() bool {
	return it.Keyword != "" && (it.Kind == KindQuicklink || strings.Contains(it.Text, "{query}"))
}

func (it Item) resultID() core.ID {
	source := core.SourceSnippet
	if it.Kind == KindQuicklink {
		source = core.SourceLink
	}
	return source.ID(strconv.FormatInt(it.ID, 10))
}
