package engine

import (
	"context"

	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

// UpdateSettings changes the settings on the writer, saves settings.json,
// and publishes them. A save that fails changes nothing. Turning clipboard
// history off moves the clipboard epoch, as SetClipboardHistory does, so
// content read before is never saved after.
func (e *Engine) UpdateSettings(file SettingsFile, change func(*settings.Settings)) <-chan error {
	return e.Change(func(next *search.Snapshot, db *store.Store) error {
		updated := next.Settings
		change(&updated)
		updated = updated.Normalize()
		if err := file.Save(updated); err != nil {
			return err
		}
		if next.Settings.ClipboardHistoryEnabled && !updated.ClipboardHistoryEnabled {
			e.clipEpoch.Add(1)
		}
		if next.Settings.ClipboardHistoryLimit != updated.ClipboardHistoryLimit && updated.ClipboardHistoryEnabled {
			ctx := context.Background()
			if err := db.TrimClipboard(ctx, updated.ClipboardHistoryLimit); err != nil {
				return err
			}
			if err := reloadClips(ctx, next, db); err != nil {
				return err
			}
		}
		next.Settings = updated
		return nil
	})
}
