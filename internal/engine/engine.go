package engine

import (
	"context"
	"errors"
	"sync/atomic"
	"time"

	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// Engine holds the published snapshot and the one goroutine that changes
// it. Readers load the snapshot without a lock; every change, from an
// action, a setting, a scan, or a copied clip, runs on the writer, one at a
// time, so no change can read a state that another is about to replace.
type Engine struct {
	current  atomic.Pointer[search.Snapshot]
	changes  chan change
	store    *store.Store
	onChange func()
	// clipEpoch counts clipboard history clears and switch-offs. Only the
	// writer moves it; see ClipEpoch.
	clipEpoch atomic.Uint64
}

type change struct {
	apply func(next *search.Snapshot, db *store.Store) error
	done  chan error
}

// errUnchanged tells the writer that a change had nothing to do, so it
// publishes nothing.
var errUnchanged = errors.New("unchanged")

// New returns an engine that starts from first. onChange runs on the writer
// after each published change; the app uses it to refresh the view.
func New(db *store.Store, first search.Snapshot, onChange func()) *Engine {
	e := &Engine{changes: make(chan change, 64), store: db, onChange: onChange}
	e.current.Store(&first)
	return e
}

// Run applies changes until ctx ends.
func (e *Engine) Run(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case c := <-e.changes:
			next := *e.current.Load()
			err := c.apply(&next, e.store)
			switch {
			case err == nil:
				e.current.Store(&next)
				if e.onChange != nil {
					e.onChange()
				}
			case errors.Is(err, errUnchanged):
				err = nil
			}
			c.done <- err
		}
	}
}

// Snapshot returns the published state with the clock set to now.
func (e *Engine) Snapshot(now time.Time) *search.Snapshot {
	s := *e.current.Load()
	s.Now = now
	return &s
}

// Change queues a change and returns where its result arrives. The main
// thread must not wait on it: the writer may be waiting on the main thread.
func (e *Engine) Change(apply func(next *search.Snapshot, db *store.Store) error) <-chan error {
	done := make(chan error, 1)
	e.changes <- change{apply, done}
	return done
}
