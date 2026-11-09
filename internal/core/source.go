package core

import "strings"

// ID identifies a result across launches: "<source prefix>:<key>". IDs are
// stored in tinydash.db (usage, pins, hidden results, aliases, hotkeys), so
// a prefix never changes.
type ID string

// Source is where a result with a lasting ID comes from.
type Source uint8

const (
	SourceApp Source = iota
	SourceFile
	SourceClip
	SourceSnippet
	SourceLink
	SourceEmoji
	SourceSystem
	sourceCount
)

// SourceTraits says what users can do with the results of a source. Each row
// is an unkeyed literal, so a new field breaks every row until it is set.
type SourceTraits struct {
	Prefix   string
	Category Category
	// LearnsFromUse: running a result ranks it higher next time.
	LearnsFromUse bool
	// Hideable, Aliasable: the actions menu offers Hide and Add Alias.
	Hideable  bool
	Aliasable bool
	// Suggestible: it may appear among "used often" in the empty All view.
	Suggestible bool
}

var sourceTraits = [sourceCount]SourceTraits{
	SourceApp:     {"app", CategoryApps, true, true, true, true},
	SourceFile:    {"file", CategoryFiles, true, true, true, true},
	SourceClip:    {"clip", CategoryClipboard, false, false, false, false},
	SourceSnippet: {"snippet", CategorySnippets, true, true, true, true},
	SourceLink:    {"link", CategorySnippets, true, true, true, true},
	SourceEmoji:   {"emoji", CategoryEmoji, true, true, true, true},
	SourceSystem:  {"system", CategorySystem, true, true, true, false},
}

// Sources lists every source.
func Sources() []Source {
	all := make([]Source, sourceCount)
	for i := range all {
		all[i] = Source(i)
	}
	return all
}

// Traits returns what users can do with the source's results.
func (s Source) Traits() SourceTraits { return sourceTraits[s] }

// ID returns the ID of the source's result with this key.
func (s Source) ID(key string) ID { return ID(sourceTraits[s].Prefix + ":" + key) }

// ParseID splits an ID into its source and key.
func ParseID(id ID) (Source, string, bool) {
	prefix, key, ok := strings.Cut(string(id), ":")
	if !ok {
		return 0, "", false
	}
	for _, s := range Sources() {
		if sourceTraits[s].Prefix == prefix {
			return s, key, true
		}
	}
	return 0, "", false
}

// Traits of the source of an ID; ok is false for an ID of no source, such as
// an instant answer's.
func (id ID) Traits() (SourceTraits, bool) {
	s, _, ok := ParseID(id)
	if !ok {
		return SourceTraits{}, false
	}
	return s.Traits(), true
}

// Category is a tab of the launcher.
type Category string

const (
	CategoryAll       Category = "all"
	CategoryApps      Category = "apps"
	CategoryFiles     Category = "files"
	CategoryClipboard Category = "clipboard"
	CategorySnippets  Category = "snippets"
	CategoryEmoji     Category = "emoji"
	CategorySystem    Category = "system"
)

// Categories lists every category in the default tab order.
func Categories() []Category {
	return []Category{CategoryAll, CategoryApps, CategoryFiles, CategoryClipboard, CategorySnippets, CategoryEmoji, CategorySystem}
}
