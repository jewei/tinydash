// Package library holds the snippets (text templates) and quicklinks (URL or
// path templates) that the user saves in Settings.
package library

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
