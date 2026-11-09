package engine

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"slices"
	"time"

	"github.com/jewei/tinydash/internal/feature/datetime"
	"github.com/jewei/tinydash/internal/feature/emoji"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

// System applies the settings that change the OS, such as the launcher
// shortcut and the login item, going from one set of settings to another.
// The app passes it; the engine calls it again from the new settings to the
// old ones to undo a change that could not be saved.
type System func(from, to settings.Settings) error

// UpdateSettings changes the settings on the writer, saves settings.json,
// and publishes them. A save that fails changes nothing. Turning clipboard
// history off moves the clipboard epoch, as SetClipboardHistory does, so
// content read before is never saved after.
func (e *Engine) UpdateSettings(file SettingsFile, change func(*settings.Settings)) <-chan error {
	return e.Change(e.settingsChange(file, change, nil, nil))
}

// UpdateSettings is the Settings window's change: Engine.UpdateSettings,
// after the checks that need the hotkeys, with system applying what
// changes the OS. Either all of it applies, or the settings, the OS, and
// settings.json stay as they were and the error says why. It waits for the
// writer, so it must not run on the main thread.
func (a *Actions) UpdateSettings(file SettingsFile, change func(*settings.Settings), system System) error {
	return a.change(a.engine.settingsChange(file, change, a.checkShortcut, system))
}

func (e *Engine) settingsChange(file SettingsFile, change func(*settings.Settings), check func(*search.Snapshot, settings.Settings) error, system System) func(*search.Snapshot, *store.Store) error {
	return func(next *search.Snapshot, db *store.Store) error {
		old := next.Settings
		updated := old
		change(&updated)
		updated = updated.Normalize()
		if err := checkSettings(old, updated, next.Home); err != nil {
			return err
		}
		if check != nil {
			if err := check(next, updated); err != nil {
				return err
			}
		}
		if err := applySettings(file, system, old, updated); err != nil {
			return err
		}
		if old.ClipboardHistoryEnabled && !updated.ClipboardHistoryEnabled {
			e.clipEpoch.Add(1)
		}
		ctx := context.Background()
		// A lower limit applies at once, also while history is off, so the
		// entries that it saved do not come back over the limit. The
		// settings are saved: a failed trim is retried by the next save of
		// a copy.
		if updated.ClipboardHistoryLimit < old.ClipboardHistoryLimit {
			err := db.TrimClipboard(ctx, updated.ClipboardHistoryLimit)
			if err == nil {
				err = reloadClips(ctx, next, db)
			}
			if err != nil {
				slog.Warn("Could not remove old clipboard entries", "error", err)
			}
		}
		if !slices.Equal(old.EmojiLanguages, updated.EmojiLanguages) {
			next.Emoji = emojiIndex(updated.EmojiLanguages)
		}
		// Rates that are off are not kept, so nothing shows when they were
		// last downloaded.
		if old.CurrencyRatesEnabled && !updated.CurrencyRatesEnabled {
			if err := db.DeleteRates(ctx); err != nil {
				slog.Warn("Could not delete the saved exchange rates", "error", err)
			}
			next.Rates = nil
		}
		next.Settings = updated
		return nil
	}
}

// applySettings changes the OS, then saves. When either fails, it undoes
// whatever part did apply; the first error is the one to show.
func applySettings(file SettingsFile, system System, old, updated settings.Settings) error {
	if system == nil {
		return file.Save(updated)
	}
	err := system(old, updated)
	if err == nil {
		err = file.Save(updated)
	}
	if err != nil {
		if undo := system(updated, old); undo != nil {
			slog.Warn("Could not undo a settings change", "error", undo)
		}
	}
	return err
}

// checkSettings refuses lists that cannot work. Each list is checked only
// when it changes, so an entry from a hand-edited file does not block other
// settings.
func checkSettings(old, updated settings.Settings, home string) error {
	if !slices.Equal(old.FileSearchFolders, updated.FileSearchFolders) {
		if folder, ok := updated.RelativeFolder(home); ok {
			return fmt.Errorf("“%s” is not a full folder path. Enter the whole path, or start it with ~ for your home folder, such as ~/Projects.", folder)
		}
	}
	if !slices.Equal(old.ClockCities, updated.ClockCities) {
		now := time.Now()
		for _, city := range updated.ClockCities {
			if _, ok := datetime.City(city, now); !ok {
				return fmt.Errorf("TinyDash does not know “%s”. Enter a city or a time zone, such as Tokyo or Europe/London.", city)
			}
		}
	}
	return nil
}

// checkShortcut refuses a launcher shortcut that would block typing or
// that a result's hotkey already has.
func (a *Actions) checkShortcut(s *search.Snapshot, updated settings.Settings) error {
	if updated.Shortcut == s.Settings.Shortcut {
		return nil
	}
	keys, err := a.hotkeys.Parse(updated.Shortcut)
	if err != nil {
		return err
	}
	if !canBeHotkey(keys) && keys != (Keys{Mods: ModShift, Key: "Space"}) {
		return errors.New("The shortcut needs Control, Alt, Command (Win or Super), or a function key, or is Shift+Space, so it never blocks typing.")
	}
	if owner, taken := hotkeyOwner(s, a.hotkeys, keys, ""); taken {
		return fmt.Errorf("These keys already run %s. Choose other keys, or remove that hotkey in Settings > Search.", owner)
	}
	return nil
}

func emojiIndex(languages []settings.EmojiLanguage) *emoji.Index {
	list := make([]emoji.Language, len(languages))
	for i, language := range languages {
		list[i] = emoji.Language(language)
	}
	return emoji.NewIndex(list...)
}
