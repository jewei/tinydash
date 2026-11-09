package app

import (
	"context"
	"errors"
	"io"
	"log/slog"
	"path/filepath"
	"slices"
	"sync"
	"sync/atomic"
	"time"

	"github.com/egoist/mygo"

	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/platform"
)

// fileWatch marks the file index dirty when names change under the indexed
// folders. It only marks: the next launcher show rebuilds, so a folder that
// changes all the time cannot keep a scan running. Where watching fails,
// the 15-minute rebuild (staleAfter) still bounds how old the index gets.
type fileWatch struct {
	// start watches one folder (mygo.WatchFiles); tests pass a fake.
	start     func(ctx context.Context, folder string, opts mygo.FileWatchOptions) (watcher, error)
	recursive bool
	packages  []string
	hidden    func(path string) bool

	// dirty: names changed since the last rebuild was asked for.
	dirty atomic.Bool

	mu sync.Mutex
	// folders and excluded are what the running watches follow.
	folders, excluded []string
	stop              context.CancelFunc
}

// watcher is what fileWatch needs of a mygo.FileWatcher.
type watcher interface {
	Next(ctx context.Context) ([]mygo.FileEvent, error)
	Close() error
}

func startMygoWatch(ctx context.Context, folder string, opts mygo.FileWatchOptions) (watcher, error) {
	return mygo.WatchFiles(ctx, folder, opts)
}

// sync watches folders, skipping excluded names, unless the watches follow
// them already. A change of either starts the watches again.
func (f *fileWatch) sync(ctx context.Context, folders, excluded []string) {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.stop != nil && slices.Equal(folders, f.folders) && slices.Equal(excluded, f.excluded) {
		return
	}
	if f.stop != nil {
		f.stop()
	}
	f.folders, f.excluded = slices.Clone(folders), slices.Clone(excluded)
	watchCtx, stop := context.WithCancel(ctx)
	f.stop = stop
	for _, folder := range f.folders {
		go f.watch(watchCtx, folder, f.excluded)
	}
}

func (f *fileWatch) watch(ctx context.Context, folder string, excluded []string) {
	folder = filepath.Clean(folder)
	w, err := f.start(ctx, folder, mygo.FileWatchOptions{
		Recursive: f.recursive,
		// What the index never lists is never watched: hidden names,
		// excluded folders, and what is inside a package. The watch has
		// asked about every folder above path already, so only its last
		// name needs the disk.
		Ignore: func(path string, isDir bool) bool {
			full := filepath.Join(folder, filepath.FromSlash(path))
			last := func(p string) bool { return p == full && f.hidden != nil && f.hidden(p) }
			return !files.Lists(full, isDir, []string{folder}, excluded, f.packages, last)
		},
	})
	if err != nil {
		if ctx.Err() == nil {
			slog.Warn("Could not watch an indexed folder; the index rebuilds when it is 15 minutes old", "folder", folder, "error", err)
		}
		return
	}
	defer w.Close()
	for {
		events, err := w.Next(ctx)
		if err != nil {
			if ctx.Err() == nil && !errors.Is(err, io.EOF) {
				// A watch that stopped may have missed a change.
				f.dirty.Store(true)
				slog.Warn("Stopped watching an indexed folder; the index rebuilds when it is 15 minutes old", "folder", folder, "error", err)
			}
			return
		}
		if changesNames(events, folder, excluded, f.packages, f.hidden) {
			f.dirty.Store(true)
		}
	}
}

// changesNames reports whether events in the watched folder root add,
// remove, or rename something that the index lists. A write changes only
// content, which the index does not keep.
func changesNames(events []mygo.FileEvent, root string, excluded, packages []string, hidden func(string) bool) bool {
	lists := func(path string, isDir bool) bool {
		full := filepath.Join(root, filepath.FromSlash(path))
		return files.Lists(full, isDir, []string{root}, excluded, packages, hidden)
	}
	for _, e := range events {
		switch e.Op {
		case mygo.FileCreate, mygo.FileRemove:
			if lists(e.Path, e.IsDir) {
				return true
			}
		case mygo.FileRename:
			if lists(e.Path, e.IsDir) || lists(e.OldPath, e.IsDir) {
				return true
			}
		}
	}
	return false
}

func newFileWatch() *fileWatch {
	return &fileWatch{start: startMygoWatch, recursive: platform.RecursiveWatch, packages: platform.PackageExtensions, hidden: platform.HiddenCheck()}
}

// watchFolders watches the folders that the settings index. It runs at
// start and after every change, and restarts the watches only when the
// folders or the names to skip changed.
func (a *App) watchFolders() {
	s := a.engine.Snapshot(time.Now())
	a.watch.sync(a.ctx, s.Settings.FileFolders(s.Home), s.Settings.FileSearchExcludedDirs)
}
