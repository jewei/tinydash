package app

import (
	"bytes"
	"context"
	"errors"
	"io"
	"io/fs"
	"log/slog"
	"sync"
	"testing"
	"time"

	"github.com/egoist/mygo"

	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/golden"
)

func TestOnlyNameChangesThatTheIndexListsMarkItDirty(t *testing.T) {
	root := golden.Abs("/home/me/Documents")
	hidden := func(path string) bool { return golden.Slash(path) == "/home/me/Documents/flagged" }
	for _, c := range []struct {
		name  string
		event mygo.FileEvent
		dirty bool
	}{
		{"a file created", mygo.FileEvent{Op: mygo.FileCreate, Path: "a.txt"}, true},
		{"a file removed deep down", mygo.FileEvent{Op: mygo.FileRemove, Path: "x/y/a.txt"}, true},
		{"a rename into view", mygo.FileEvent{Op: mygo.FileRename, Path: "a.txt", OldPath: ".a.txt"}, true},
		{"a rename out of view", mygo.FileEvent{Op: mygo.FileRename, Path: ".a.txt", OldPath: "a.txt"}, true},
		{"the folder itself went away", mygo.FileEvent{Op: mygo.FileRemove, Path: ".", IsDir: true}, true},
		{"a write", mygo.FileEvent{Op: mygo.FileWrite, Path: "a.txt"}, false},
		{"a dot name", mygo.FileEvent{Op: mygo.FileCreate, Path: ".cache/a"}, false},
		{"in a folder to skip", mygo.FileEvent{Op: mygo.FileCreate, Path: "node_modules/a"}, false},
		{"a folder to skip itself", mygo.FileEvent{Op: mygo.FileCreate, Path: "node_modules", IsDir: true}, false},
		{"a file named like a folder to skip", mygo.FileEvent{Op: mygo.FileCreate, Path: "node_modules"}, true},
		{"inside a package", mygo.FileEvent{Op: mygo.FileCreate, Path: "Tool.app/Contents", IsDir: true}, false},
		{"a package itself", mygo.FileEvent{Op: mygo.FileCreate, Path: "Tool.app", IsDir: true}, true},
		{"hidden by the OS", mygo.FileEvent{Op: mygo.FileCreate, Path: "flagged/a.txt"}, false},
	} {
		got := changesNames([]mygo.FileEvent{c.event}, root, []string{"node_modules"}, []string{"app"}, hidden)
		if got != c.dirty {
			t.Errorf("%s: dirty %v, want %v", c.name, got, c.dirty)
		}
	}
}

// fakeWatcher stands in for a mygo.FileWatcher: Next returns each batch
// sent on events, and then end's error. events has no buffer, so a send
// returns only once the watch loop asks for the next batch.
type fakeWatcher struct {
	events chan []mygo.FileEvent
	end    error
	closed chan struct{}
}

func (w *fakeWatcher) Next(ctx context.Context) ([]mygo.FileEvent, error) {
	select {
	case events, ok := <-w.events:
		if !ok {
			return nil, w.end
		}
		return events, nil
	case <-ctx.Done():
		return nil, io.EOF
	}
}

func (w *fakeWatcher) Close() error {
	close(w.closed)
	return nil
}

type fakeWatches struct {
	mu      sync.Mutex
	started map[string]*fakeWatcher
	opts    map[string]mygo.FileWatchOptions
	order   chan string
}

func newFakeWatches() *fakeWatches {
	return &fakeWatches{started: map[string]*fakeWatcher{}, opts: map[string]mygo.FileWatchOptions{}, order: make(chan string, 8)}
}

func (f *fakeWatches) start(ctx context.Context, folder string, opts mygo.FileWatchOptions) (watcher, error) {
	w := &fakeWatcher{events: make(chan []mygo.FileEvent), closed: make(chan struct{}), end: io.EOF}
	f.mu.Lock()
	f.started[golden.Slash(folder)], f.opts[golden.Slash(folder)] = w, opts
	f.mu.Unlock()
	f.order <- golden.Slash(folder)
	return w, nil
}

func (f *fakeWatches) get(folder string) (*fakeWatcher, mygo.FileWatchOptions) {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.started[folder], f.opts[folder]
}

func TestWatchesFollowTheIndexedFoldersAndMarkTheIndexDirty(t *testing.T) {
	watches := newFakeWatches()
	hidden := func(path string) bool { return golden.Slash(path) == "/home/me/Documents/flagged" }
	f := &fileWatch{start: watches.start, recursive: true, packages: []string{"app"}, hidden: hidden}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	docs := golden.Abs("/home/me/Documents")
	f.sync(ctx, []string{docs}, []string{"node_modules"})
	if got := <-watches.order; got != "/home/me/Documents" {
		t.Fatalf("watched %q", got)
	}
	w, opts := watches.get("/home/me/Documents")
	if !opts.Recursive {
		t.Error("the watch is not recursive")
	}
	for _, c := range []struct {
		path   string
		isDir  bool
		ignore bool
	}{
		{"a.txt", false, false},
		{"projects", true, false},
		{".git", true, true},
		{"node_modules", true, true},
		{"Tool.app", true, false},
		{"Tool.app/Contents", true, true},
		{"flagged", true, true},
	} {
		if got := opts.Ignore(c.path, c.isDir); got != c.ignore {
			t.Errorf("Ignore(%q) = %v, want %v", c.path, got, c.ignore)
		}
	}

	// The same settings keep the watch; a write leaves the index clean.
	f.sync(ctx, []string{docs}, []string{"node_modules"})
	w.events <- []mygo.FileEvent{{Op: mygo.FileWrite, Path: "a.txt"}}
	w.events <- nil // the loop handled the batch before once it takes this
	if f.dirty.Load() {
		t.Error("a write marked the index dirty")
	}
	w.events <- []mygo.FileEvent{{Op: mygo.FileCreate, Path: "b.txt"}}
	w.events <- nil
	if !f.dirty.Load() {
		t.Error("a created file left the index clean")
	}

	// Other folders stop the old watch and start new ones.
	f.sync(ctx, []string{golden.Abs("/home/me/Desktop")}, []string{"node_modules"})
	<-w.closed
	if got := <-watches.order; got != "/home/me/Desktop" {
		t.Errorf("watched %q, want the new folder", got)
	}
}

func TestAWatchThatFailsMarksTheIndexDirty(t *testing.T) {
	watches := newFakeWatches()
	f := &fileWatch{start: watches.start}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	f.sync(ctx, []string{golden.Abs("/home/me/Documents")}, nil)
	<-watches.order
	w, _ := watches.get("/home/me/Documents")
	w.end = errors.New("too many files")
	close(w.events)
	<-w.closed
	if !f.dirty.Load() {
		t.Error("a stopped watch left the index clean")
	}
}

func TestADirtyFileIndexRebuildsAtTheNextShow(t *testing.T) {
	built := make(chan struct{}, 4)
	a := &App{builtAt: map[*engine.Slot]time.Time{}, watch: &fileWatch{}}
	a.files = engine.NewSlot(func(context.Context) { built <- struct{}{} })
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go a.files.Run(ctx)
	a.rebuild(a.files)
	<-built
	a.rebuildIfOld(a.files)
	a.watch.dirty.Store(true)
	a.rebuildIfOld(a.files)
	<-built
	if a.watch.dirty.Load() {
		t.Error("the rebuild left the index dirty")
	}
	if n := len(built); n != 0 {
		t.Errorf("%d rebuilds of a fresh, clean index", n)
	}
}

func TestAMissingFolderIsSkippedWithoutAWarning(t *testing.T) {
	var logged bytes.Buffer
	defer slog.SetDefault(slog.Default())
	slog.SetDefault(slog.New(slog.NewTextHandler(&logged, nil)))
	f := &fileWatch{start: func(context.Context, string, mygo.FileWatchOptions) (watcher, error) {
		// What mygo.WatchFiles returns for a folder that does not exist.
		return nil, &fs.PathError{Op: "lstat", Path: "/home/me/gone", Err: fs.ErrNotExist}
	}}
	f.watch(context.Background(), golden.Abs("/home/me/gone"), nil)
	if logged.Len() != 0 || f.dirty.Load() {
		t.Errorf("dirty %v, logged %q", f.dirty.Load(), logged.String())
	}
}

// A change in an app folder marks the app index dirty, as a change in an
// indexed folder does the file index; the next show rebuilds it.
func TestADirtyAppIndexRebuildsAtTheNextShow(t *testing.T) {
	watches := newFakeWatches()
	built := make(chan struct{}, 4)
	a := &App{builtAt: map[*engine.Slot]time.Time{}, watch: &fileWatch{}, appWatch: &fileWatch{start: watches.start, packages: []string{"app"}}}
	a.apps = engine.NewSlot(func(context.Context) { built <- struct{}{} })
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	go a.apps.Run(ctx)
	a.rebuild(a.apps)
	<-built

	a.appWatch.sync(ctx, []string{golden.Abs("/Applications")}, nil)
	<-watches.order
	w, opts := watches.get("/Applications")
	if !opts.Ignore("Tool.app/Contents", true) || opts.Ignore("Tool.app", true) {
		t.Error("the app watch follows the inside of bundles, or not the bundles")
	}
	w.events <- []mygo.FileEvent{{Op: mygo.FileCreate, Path: "Tool.app", IsDir: true}}
	w.events <- nil
	a.rebuildIfOld(a.apps)
	select {
	case <-built:
	case <-time.After(5 * time.Second):
		t.Fatal("a dirty app index was not rebuilt at the next show")
	}
	if a.appWatch.dirty.Load() {
		t.Error("the rebuild left the app index dirty")
	}
	a.rebuildIfOld(a.apps)
	if n := len(built); n != 0 {
		t.Errorf("%d rebuilds of a fresh, clean app index", n)
	}
}
