package engine

import (
	"context"
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// Pin, Hide, SetAlias, and SetHotkey check their limits on the writer,
// against the state they change, so two quick changes never both pass a
// check. Each one stores the item's own ID, as Resolve gives it, so a pin
// never widens what Open and Reveal accept.

// Keys is a key combination: modifiers and one key, as HotkeyRegistry.Parse
// reads accelerator text. Equal keys are the same combination.
type Keys struct {
	Mods Modifiers
	// Key is the key's code, such as KeyL, Space, or F5.
	Key string
}

// Modifiers are the modifier keys of a combination.
type Modifiers uint8

const (
	ModShift Modifiers = 1 << iota
	ModControl
	ModAlt
	// ModSuper is Command on macOS and the Windows key elsewhere.
	ModSuper
)

// HotkeyRegistry registers hotkeys with the OS. The app passes its global
// shortcut registry, which also holds the launcher shortcut.
type HotkeyRegistry interface {
	// Parse reads accelerator text, such as Control+Shift+Space. Its
	// error is a sentence for the user.
	Parse(accelerator string) (Keys, error)
	// Sync makes the registered hotkeys match wanted: it releases the ones
	// wanted lacks or changed and registers the new ones. It returns the
	// error of each hotkey that could not be changed, by result ID; the
	// others still apply.
	Sync(wanted search.Hotkeys) map[core.ID]error
}

// canBeHotkey reports whether keys can be a hotkey without blocking typing
// everywhere: they hold Control, Alt, or Super, or are a function key.
func canBeHotkey(keys Keys) bool {
	if keys.Mods&(ModControl|ModAlt|ModSuper) != 0 {
		return true
	}
	n, ok := strings.CutPrefix(keys.Key, "F")
	if !ok {
		return false
	}
	_, err := strconv.ParseUint(n, 10, 8)
	return err == nil
}

// CanOpenLauncher reports whether keys can be the launcher shortcut: the
// keys a hotkey can be, and also Shift+Space, as the recorder takes.
func CanOpenLauncher(keys Keys) bool {
	return canBeHotkey(keys) || keys == (Keys{Mods: ModShift, Key: "Space"})
}

var errGone = errors.New("This item no longer exists.")

// Pin pins an item that exists and is not hidden.
func (h handler) Pin(a core.Pin) error {
	return h.write(func(next *search.Snapshot, db *store.Store) error {
		found, ok := search.Resolve(next, a.ID)
		if !ok {
			return errGone
		}
		id := found.ID
		// A hidden pin would hold a place in the list no one can see.
		if next.Hidden.Contains(id) {
			return errors.New("This item is hidden. Show it again in Settings > Search, then pin it.")
		}
		oldest, drop, err := pinToDrop(next, id)
		if err != nil {
			return err
		}
		ctx := context.Background()
		wrote := false
		if drop {
			if err := db.SetPinned(ctx, oldest, false); err != nil {
				return err
			}
			next.Pins = next.Pins.Remove(oldest)
			wrote = true
		}
		if err := db.SetPinned(ctx, id, true); err != nil {
			if wrote {
				return afterWrite{err}
			}
			return err
		}
		next.Pins = next.Pins.Add(id)
		return nil
	})
}

// pinToDrop is the pin that must go before id fits; drop is false while
// there is room or id is pinned already.
//
// A pin of a hidden item (a file outside the index, or a clipboard entry
// while history is off) waits for its item to return, but it cannot be
// unpinned, so it gives up its place when the list is full. That way no
// more than MaxPins pins ever show. Until a source is ready, its pins only
// look hidden, so they stay. A pin of an item the user hid shows in no
// list either; one can remain from before Pin refused hidden items.
func pinToDrop(s *search.Snapshot, id core.ID) (oldest core.ID, drop bool, err error) {
	if s.Pins.Len() < search.MaxPins || s.Pins.Contains(id) {
		return "", false, nil
	}
	for _, pin := range s.Pins.IDs() {
		source, _, parsed := core.ParseID(pin)
		known := !parsed || s.Ready(source)
		if s.Hidden.Contains(pin) {
			return pin, true, nil
		}
		if _, ok := search.Resolve(s, pin); known && !ok {
			return pin, true, nil
		}
	}
	return "", false, fmt.Errorf("You can pin up to %d items.", search.MaxPins)
}

// Hide hides an item that exists, under its own ID. A row for a deleted
// item would stay in Settings and count toward the limit.
func (h handler) Hide(a core.Hide) error {
	now := h.now().Unix()
	return h.write(func(next *search.Snapshot, db *store.Store) error {
		if traits, ok := a.ID.Traits(); !ok || !traits.Hideable {
			return errors.New("This result cannot be hidden.")
		}
		found, ok := search.Resolve(next, a.ID)
		if !ok {
			return errGone
		}
		id := found.ID
		if next.Hidden.Len() >= search.MaxHidden {
			return fmt.Errorf("You can hide up to %d results. Show some again in Settings > Search.", search.MaxHidden)
		}
		ctx := context.Background()
		if err := db.SetHidden(ctx, id, true, now); err != nil {
			return err
		}
		next.Hidden = next.Hidden.Add(id)
		// A hidden pin would hold a place in the list no one can see.
		if err := db.SetPinned(ctx, id, false); err != nil {
			return afterWrite{err}
		}
		next.Pins = next.Pins.Remove(id)
		return nil
	})
}

// SetAlias gives a result an alias, or removes its alias when the alias is
// empty. Only a result that exists gets one, and an alias names one result.
func (h handler) SetAlias(a core.SetAlias) error {
	return h.change(func(next *search.Snapshot, db *store.Store) error {
		if traits, ok := a.ID.Traits(); !ok || !traits.Aliasable {
			return errors.New("This result cannot have an alias.")
		}
		alias, err := search.CleanAlias(a.Alias)
		if err != nil {
			return err
		}
		ctx := context.Background()
		found, ok := search.Resolve(next, a.ID)
		if alias == "" {
			// The item may be gone; its alias can still be removed.
			id := a.ID
			if ok {
				id = found.ID
			}
			if err := db.RemoveAlias(ctx, id); err != nil {
				return err
			}
			next.Aliases = next.Aliases.Remove(id)
			return nil
		}
		if !ok {
			return errGone
		}
		id := found.ID
		if owner, taken := next.Aliases.Owner(alias); taken && owner != id {
			return fmt.Errorf("“%s” is already the alias of %s. Choose another word.", alias, name(next, owner))
		}
		if _, has := next.Aliases.Get(id); !has && next.Aliases.Len() >= search.MaxAliases {
			return fmt.Errorf("You can set up to %d aliases. Remove some in Settings > Search.", search.MaxAliases)
		}
		if err := db.SetAlias(ctx, id, alias); err != nil {
			return err
		}
		next.Aliases = next.Aliases.Set(id, alias)
		return nil
	})
}

// SetHotkey gives a result a hotkey, or removes it when the shortcut is
// empty. The keys must register, and neither the launcher nor another
// result may have them. Either everything changes, or nothing does and the
// error says why.
func (h handler) SetHotkey(a core.SetHotkey) error {
	return h.change(func(next *search.Snapshot, db *store.Store) error {
		id, shortcut, err := checkedHotkey(next, h.hotkeys, a.ID, a.Shortcut)
		if err != nil {
			return err
		}
		current := next.Hotkeys
		wanted := current.Remove(id)
		if shortcut != "" {
			wanted = current.Set(id, shortcut)
		}
		// Register first: keys another app holds must not be saved.
		if err := h.hotkeys.Sync(wanted)[id]; err != nil {
			h.hotkeys.Sync(current)
			return err
		}
		ctx := context.Background()
		if shortcut == "" {
			err = db.RemoveHotkey(ctx, id)
		} else {
			err = db.SetHotkey(ctx, id, shortcut)
		}
		if err != nil {
			h.hotkeys.Sync(current)
			return err
		}
		next.Hotkeys = wanted
		return nil
	})
}

// checkedHotkey returns the result ID that SetHotkey changes, and its keys,
// or "" to remove them. It refuses keys that cannot be a hotkey for it.
func checkedHotkey(s *search.Snapshot, registry HotkeyRegistry, id core.ID, shortcut string) (core.ID, string, error) {
	if traits, ok := id.Traits(); !ok || !traits.Aliasable {
		return "", "", errors.New("This result cannot have a hotkey.")
	}
	found, ok := search.Resolve(s, id)
	shortcut = strings.TrimSpace(shortcut)
	if shortcut == "" {
		// The item may be gone; its hotkey can still be removed.
		if ok {
			id = found.ID
		}
		return id, "", nil
	}
	if !ok {
		return "", "", errGone
	}
	if len(found.Actions) == 0 || found.Actions[0].Confirm != "" {
		return "", "", fmt.Errorf("%s asks before it runs, so it cannot have a hotkey.", found.Title)
	}
	keys, err := registry.Parse(shortcut)
	if err != nil {
		return "", "", err
	}
	if !canBeHotkey(keys) {
		return "", "", errors.New("A hotkey needs a modifier such as Control or Alt, or a function key, so it never blocks typing.")
	}
	if launcher, err := registry.Parse(s.Settings.Shortcut); err == nil && launcher == keys {
		return "", "", errors.New("These keys show TinyDash. Choose other keys.")
	}
	if owner, taken := hotkeyOwner(s, registry, keys, found.ID); taken {
		return "", "", fmt.Errorf("These keys already run %s. Choose other keys.", owner)
	}
	if _, has := s.Hotkeys.Get(found.ID); !has && s.Hotkeys.Len() >= search.MaxHotkeys {
		return "", "", fmt.Errorf("You can set up to %d hotkeys. Remove some in Settings > Search.", search.MaxHotkeys)
	}
	return found.ID, shortcut, nil
}

// hotkeyOwner is the name of the result whose hotkey is keys, other than
// except.
func hotkeyOwner(s *search.Snapshot, registry HotkeyRegistry, keys Keys, except core.ID) (string, bool) {
	for _, hotkey := range s.Hotkeys.All() {
		if hotkey.ID == except {
			continue
		}
		if other, err := registry.Parse(hotkey.Value); err == nil && other == keys {
			return name(s, hotkey.ID), true
		}
	}
	return "", false
}

// name is the title of the result with id, or the ID when it is gone.
func name(s *search.Snapshot, id core.ID) string {
	if found, ok := search.Resolve(s, id); ok {
		return found.Title
	}
	return string(id)
}

// HotkeyOff is the warning for a hotkey that did not register at startup.
// It names the result, and apps and files, not scanned yet, by their path.
func HotkeyOff(s *search.Snapshot, id core.ID, err error) string {
	title := string(id)
	if found, ok := search.Resolve(s, id); ok {
		title = found.Title
	} else if _, key, ok := core.ParseID(id); ok {
		title = key
	}
	return fmt.Sprintf("The hotkey of %s is off. %v Use Set Hotkey in its actions, or remove it in Settings > Search.", title, err)
}

// hotkeyAction is the main action of the result with id. Until a source is
// ready, its items only look gone: a hotkey pressed just after startup must
// not ask the user to remove it.
func hotkeyAction(s *search.Snapshot, id core.ID) (core.Action, error) {
	found, ok := search.Resolve(s, id)
	if !ok {
		if source, _, parsed := core.ParseID(id); parsed && !s.Ready(source) {
			return nil, errors.New("TinyDash is still loading your apps and files. Try again in a moment.")
		}
		return nil, errors.New("This hotkey's item no longer exists. Remove the hotkey in Settings > Search.")
	}
	if len(found.Actions) == 0 || found.Actions[0].Confirm != "" {
		return nil, errors.New("This item asks before it runs. Run it from TinyDash.")
	}
	return found.Actions[0].Action, nil
}
