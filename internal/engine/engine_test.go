package engine

import (
	"context"
	"sync/atomic"
	"testing"

	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

// start runs an engine over an in-memory store with clipboard history on.
func start(t *testing.T) (*Engine, *atomic.Int32) {
	t.Helper()
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)
	db, err := store.OpenMemory(ctx)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	first := search.Snapshot{Settings: settings.Default(), Clipboard: clipboard.NewHistory(nil)}
	first.Settings.ClipboardHistoryEnabled = true
	var changes atomic.Int32
	e := New(db, first, func() { changes.Add(1) })
	go e.Run(ctx)
	return e, &changes
}

func clips(t *testing.T, e *Engine) int {
	t.Helper()
	return len(e.Snapshot(e.current.Load().Now).Clipboard.Browse(clipboard.Context{}))
}

func TestAChangeIsPublishedAndAnnounced(t *testing.T) {
	e, changes := start(t)
	if err := <-e.SaveClip(e.ClipEpoch(), clipboard.Text("hello"), 100); err != nil {
		t.Fatal(err)
	}
	if clips(t, e) != 1 || changes.Load() != 1 {
		t.Fatalf("%d clips, %d announcements", clips(t, e), changes.Load())
	}
	// Nothing to save: nothing published, nothing announced.
	if err := <-e.SaveClip(e.ClipEpoch(), clipboard.Text("   "), 101); err != nil {
		t.Fatal(err)
	}
	if changes.Load() != 1 {
		t.Fatalf("%d announcements after a change with nothing to do", changes.Load())
	}
}

// The monitor read the clipboard, then the user cleared the history before
// the save ran: the old content must not come back.
func TestContentReadBeforeAClearIsNeverSaved(t *testing.T) {
	e, _ := start(t)
	epoch := e.ClipEpoch() // the monitor reads the epoch, then the clipboard
	if err := <-e.ClearClipboard(); err != nil {
		t.Fatal(err)
	}
	if err := <-e.SaveClip(epoch, clipboard.Text("secret"), 100); err != nil {
		t.Fatal(err)
	}
	if n := clips(t, e); n != 0 {
		t.Fatalf("%d clips after a clear", n)
	}
	if err := <-e.SaveClip(e.ClipEpoch(), clipboard.Text("new"), 101); err != nil {
		t.Fatal(err)
	}
	if n := clips(t, e); n != 1 {
		t.Fatalf("a copy after the clear: %d clips", n)
	}
}

func TestContentReadBeforeHistoryWasTurnedOffIsNeverSaved(t *testing.T) {
	e, _ := start(t)
	epoch := e.ClipEpoch()
	setHistory(t, e, false)
	setHistory(t, e, true)
	if err := <-e.SaveClip(epoch, clipboard.Text("secret"), 100); err != nil {
		t.Fatal(err)
	}
	if n := clips(t, e); n != 0 {
		t.Fatalf("%d clips from before history was off", n)
	}
}
