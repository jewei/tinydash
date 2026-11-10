package engine

import (
	"context"
	"math/rand/v2"
	"path/filepath"
	"testing"
	"time"

	"github.com/egoist/mygo/plugins/sqlite"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

// unreadableHistory starts a world whose saved history holds "kept", which
// is pinned, and "secret". Then the pinned row breaks, so every read of the
// history fails while deletes still work.
func unreadableHistory(t *testing.T) (w *world, secret core.ID) {
	t.Helper()
	ctx := context.Background()
	dir := t.TempDir()
	db, _, err := store.Open(ctx, dir, func(string) error { return nil })
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	if err := db.SaveClip(ctx, clipboard.Text("kept"), 1, 100); err != nil {
		t.Fatal(err)
	}
	if err := db.SaveClip(ctx, clipboard.Text("secret"), 2, 100); err != nil {
		t.Fatal(err)
	}
	if err := db.SetPinned(ctx, clipID(1), true); err != nil {
		t.Fatal(err)
	}
	first, warnings := Load(ctx, db, settings.Default(), home, rand.NewChaCha8([32]byte{1}))
	if len(warnings) > 0 {
		t.Fatal(warnings)
	}
	raw, err := sqlite.Open(ctx, filepath.Join(dir, store.FileName), sqlite.OpenOptions{})
	if err != nil {
		t.Fatal(err)
	}
	defer raw.Close()
	for _, sql := range []string{"PRAGMA ignore_check_constraints = ON", "UPDATE clipboard SET kind = 'broken' WHERE id = 1"} {
		if _, err := raw.Execute(ctx, sql); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := db.ClipboardHistory(ctx); err == nil {
		t.Fatal("the history still reads")
	}
	run, stop := context.WithCancel(ctx)
	t.Cleanup(stop)
	e := New(db, first, nil)
	go e.Run(run)
	log := &steps{}
	w = &world{t: t, engine: e, host: &fakeOS{steps: log}, keys: &fakeKeys{}, steps: log, db: db}
	w.actions = NewActions(e, w.host, fakeShell{log}, w.keys, func() time.Time { return testNow })
	return w, clipID(2)
}

func shows(w *world, id core.ID) bool {
	for _, r := range w.engine.Snapshot(testNow).Clipboard.Browse(clipboard.Context{}) {
		if r.ID == id {
			return true
		}
	}
	return false
}

// Cleared entries leave the launcher even when the history cannot be read
// back after the clear: the rows are gone, so showing them would keep
// private text on screen.
func TestAClearHidesTheEntriesEvenWhenTheHistoryCannotBeReadBack(t *testing.T) {
	w, secret := unreadableHistory(t)
	if !shows(w, secret) {
		t.Fatal("the entry does not show before the clear")
	}
	w.run(core.ClearClipboard{}, "")
	if shows(w, secret) {
		t.Error("the cleared entry still shows")
	}
	if !shows(w, clipID(1)) {
		t.Error("the pinned entry went")
	}
}

func TestADeleteHidesTheEntryEvenWhenTheHistoryCannotBeReadBack(t *testing.T) {
	w, secret := unreadableHistory(t)
	w.run(core.DeleteClip{ID: 2}, "")
	if shows(w, secret) {
		t.Error("the deleted entry still shows")
	}
}
