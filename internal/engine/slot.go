// Package engine owns the live state: the published search snapshot, the
// single writer that changes it, and the background work that feeds it.
package engine

import (
	"context"
	"sync/atomic"
)

// Slot runs the rebuilds of one index, such as the file index, one at a
// time. Request never blocks. Requests that arrive while a rebuild runs
// cause exactly one more rebuild, which starts after all of them, so no
// request is lost and none waits behind a queue of stale ones.
type Slot struct {
	kick  chan struct{}
	build func(context.Context)
	built atomic.Bool
}

// NewSlot returns a slot that runs build for each rebuild. Start it with Run.
func NewSlot(build func(context.Context)) *Slot {
	return &Slot{kick: make(chan struct{}, 1), build: build}
}

// Request asks for a rebuild that starts after this call.
func (s *Slot) Request() {
	select {
	case s.kick <- struct{}{}:
	default: // one is already pending, and it has not started yet
	}
}

// Run rebuilds on each request until ctx ends.
func (s *Slot) Run(ctx context.Context) {
	for {
		select {
		case <-ctx.Done():
			return
		case <-s.kick:
			s.build(ctx)
			s.built.Store(true)
		}
	}
}

// Ready reports whether a rebuild has finished. Until then, search shows
// that the source is still loading rather than no results.
func (s *Slot) Ready() bool { return s.built.Load() }
