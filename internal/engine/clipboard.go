package engine

import (
	"context"
	"strconv"

	"github.com/jewei/tinydash/internal/core"

	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// ClipEpoch is the clipboard history's generation. The monitor reads it
// before it reads the clipboard and passes it to SaveClip, so content read
// before a Clear or a switch-off is never saved after it.
func (e *Engine) ClipEpoch() uint64 { return e.clipEpoch.Load() }

// SaveClip saves content that the monitor read during epoch.
func (e *Engine) SaveClip(epoch uint64, content clipboard.Content, now int64) <-chan error {
	return e.Change(func(next *search.Snapshot, db *store.Store) error {
		if epoch != e.clipEpoch.Load() || !next.Settings.ClipboardHistoryEnabled || !content.Fits() {
			return errUnchanged
		}
		ctx := context.Background()
		if err := db.SaveClip(ctx, content, now, next.Settings.ClipboardHistoryLimit); err != nil {
			return err
		}
		return reloadClips(ctx, next, db)
	})
}

// ClearClipboard deletes the clipboard history, except pinned entries.
func (e *Engine) ClearClipboard() <-chan error {
	return e.Change(func(next *search.Snapshot, db *store.Store) error {
		e.clipEpoch.Add(1)
		ctx := context.Background()
		if err := db.ClearClipboard(ctx); err != nil {
			return err
		}
		// The rows are gone: drop them here too, without reading the
		// history back, so a failed read cannot keep them on screen.
		next.Clipboard = next.Clipboard.Keep(func(entry clipboard.Entry) bool {
			return next.Pins.Contains(clipID(entry.ID))
		})
		return nil
	})
}

func reloadClips(ctx context.Context, next *search.Snapshot, db *store.Store) error {
	entries, err := db.ClipboardHistory(ctx)
	if err != nil {
		return err
	}
	next.Clipboard = clipboard.NewHistory(entries)
	return nil
}

func clipID(id int64) core.ID { return core.SourceClip.ID(strconv.FormatInt(id, 10)) }
