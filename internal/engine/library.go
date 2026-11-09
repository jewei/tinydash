package engine

import (
	"cmp"
	"context"
	"fmt"
	"log/slog"
	"slices"
	"strconv"
	"strings"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// The Settings window's changes to the library and the hidden results. Like
// Run, each waits for the writer, so none may run on the main thread.

// SaveLibraryItem checks a snippet or quicklink, saves it, and returns it as
// saved, with its ID. An item without an ID is added.
func (a *Actions) SaveLibraryItem(item library.Item) (library.Item, error) {
	now := a.now()
	var saved library.Item
	err := a.change(func(next *search.Snapshot, db *store.Store) error {
		items := next.Library.Items()
		if item.ID == 0 && len(items) >= library.MaxItems {
			return fmt.Errorf("The library holds up to %d snippets and quicklinks.", library.MaxItems)
		}
		checked, err := item.Validated(now, next.Home)
		if err != nil {
			return err
		}
		if err := next.Library.CheckKind(checked); err != nil {
			return err
		}
		if err := next.Library.CheckKeyword(checked); err != nil {
			return err
		}
		if saved, err = db.SaveLibraryItem(context.Background(), checked); err != nil {
			return err
		}
		next.Library = library.NewIndex(append(without(items, saved.ID), saved))
		return nil
	})
	return saved, err
}

// DeleteLibraryItem deletes a snippet or quicklink with its pin, usage,
// alias, hotkey, and hidden mark, and releases its hotkey.
func (a *Actions) DeleteLibraryItem(id int64) error {
	return a.change(func(next *search.Snapshot, db *store.Store) error {
		if err := db.DeleteLibraryItem(context.Background(), id); err != nil {
			return err
		}
		key := strconv.FormatInt(id, 10)
		for _, gone := range []core.ID{core.SourceSnippet.ID(key), core.SourceLink.ID(key)} {
			next.Pins = next.Pins.Remove(gone)
			next.Usage = next.Usage.Remove(gone)
			next.Hidden = next.Hidden.Remove(gone)
			next.Aliases = next.Aliases.Remove(gone)
			next.Hotkeys = next.Hotkeys.Remove(gone)
		}
		// Its row is gone; keys the OS kept only say the item is gone.
		for hotkey, err := range a.hotkeys.Sync(next.Hotkeys) {
			slog.Warn("A hotkey is off", "id", hotkey, "error", err)
		}
		next.Library = library.NewIndex(without(next.Library.Items(), id))
		return nil
	})
}

func without(items []library.Item, id int64) []library.Item {
	return slices.DeleteFunc(slices.Clone(items), func(it library.Item) bool { return it.ID == id })
}

// Unhide shows a hidden result in search again.
func (a *Actions) Unhide(id core.ID) error {
	now := a.now().Unix()
	return a.change(func(next *search.Snapshot, db *store.Store) error {
		if err := db.SetHidden(context.Background(), id, false, now); err != nil {
			return err
		}
		next.Hidden = next.Hidden.Remove(id)
		return nil
	})
}

// Listed is a result as a Settings list shows it.
type Listed struct {
	ID              core.ID
	Title, Subtitle string
	// Value is the alias or the hotkey's accelerator text, in those lists.
	Value string
}

// HiddenResults lists the results the user hid, by title. One whose item is
// gone, such as an uninstalled app, shows its key; until its source is
// ready, an item only looks gone, and says so.
func HiddenResults(s *search.Snapshot) []Listed {
	out := make([]Listed, 0, s.Hidden.Len())
	for _, id := range s.Hidden.IDs() {
		out = append(out, describe(s, id))
	}
	slices.SortStableFunc(out, func(a, b Listed) int { return cmp.Compare(strings.ToLower(a.Title), strings.ToLower(b.Title)) })
	return out
}

// AliasResults lists the aliases the user set, by alias.
func AliasResults(s *search.Snapshot) []Listed {
	out := withValues(s, s.Aliases.All())
	slices.SortStableFunc(out, func(a, b Listed) int { return cmp.Compare(a.Value, b.Value) })
	return out
}

// HotkeyResults lists the hotkeys the user set, by title.
func HotkeyResults(s *search.Snapshot) []Listed {
	out := withValues(s, s.Hotkeys.All())
	slices.SortStableFunc(out, func(a, b Listed) int { return cmp.Compare(strings.ToLower(a.Title), strings.ToLower(b.Title)) })
	return out
}

func withValues(s *search.Snapshot, pairs []search.Pair) []Listed {
	out := make([]Listed, 0, len(pairs))
	for _, pair := range pairs {
		r := describe(s, pair.ID)
		r.Value = pair.Value
		out = append(out, r)
	}
	return out
}

func describe(s *search.Snapshot, id core.ID) Listed {
	if found, ok := search.Resolve(s, id); ok {
		return Listed{ID: id, Title: found.Title, Subtitle: found.Subtitle}
	}
	source, key, ok := core.ParseID(id)
	switch {
	case !ok:
		return Listed{ID: id, Title: string(id), Subtitle: "No longer found"}
	case !s.Ready(source):
		return Listed{ID: id, Title: key, Subtitle: "Still loading"}
	}
	return Listed{ID: id, Title: key, Subtitle: "No longer found"}
}
