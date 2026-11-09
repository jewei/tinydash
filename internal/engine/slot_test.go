package engine

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
)

// Requests made during a rebuild collapse into exactly one more rebuild,
// and it starts after the last of them.
func TestRequestsDuringARebuildCauseOneMore(t *testing.T) {
	started := make(chan int, 10)
	release := make(chan struct{})
	var builds atomic.Int32
	slot := NewSlot(func(context.Context) {
		started <- int(builds.Add(1))
		<-release
	})
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan struct{})
	go func() { slot.Run(ctx); close(done) }()

	slot.Request()
	if n := <-started; n != 1 {
		t.Fatalf("first build %d", n)
	}
	if slot.Ready() {
		t.Fatal("ready before the first build finished")
	}
	for range 5 {
		slot.Request()
	}
	release <- struct{}{}
	if n := <-started; n != 2 {
		t.Fatalf("second build %d", n)
	}
	if !slot.Ready() {
		t.Fatal("not ready after a build finished")
	}
	release <- struct{}{}
	cancel()
	<-done
	if n := builds.Load(); n != 2 {
		t.Fatalf("%d builds for 1 request and 5 during it, want 2", n)
	}
}

// Many goroutines may request at once without blocking or racing.
func TestRequestNeverBlocks(t *testing.T) {
	slot := NewSlot(func(context.Context) {})
	var wg sync.WaitGroup
	for range 100 {
		wg.Go(slot.Request)
	}
	wg.Wait()
}
