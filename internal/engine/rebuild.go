package engine

import (
	"context"
	"log/slog"

	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// FileDisk is how the file index reads the disk. The app passes the real
// disk and the OS's rules.
type FileDisk struct {
	Disk files.Disk
	// Packages are extensions of folders listed without their contents,
	// such as "app".
	Packages []string
	// Hidden is the OS check for hidden items, or nil when it has none.
	Hidden func(path string) bool
}

// RebuildApps returns the work of the app index's Slot: list the installed
// apps and publish them. list is the app layer's listing, such as mygo's
// Shell.InstalledApps. When it fails, the apps found before stay.
func (e *Engine) RebuildApps(list func() ([]apps.App, error)) func(context.Context) {
	return func(ctx context.Context) {
		found, err := list()
		var index *apps.Index
		if err != nil {
			slog.Warn("Could not list the installed apps", "error", err)
		} else {
			index = apps.NewIndex(found)
			if index.Truncated {
				slog.Warn("The app index is full", "limit", apps.MaxApps)
			}
			slog.Info("Indexed applications", "apps", index.Len())
		}
		e.publish(ctx, func(next *search.Snapshot) {
			if index != nil {
				next.Apps = index
			}
			// Ready even after a failure: the launcher shows what it has
			// rather than "loading" for good.
			next.AppsReady = true
		})
	}
}

// RebuildFiles returns the work of the file index's Slot: scan the folders
// that the settings name and publish the index.
func (e *Engine) RebuildFiles(disk FileDisk) func(context.Context) {
	return func(ctx context.Context) {
		s := e.current.Load()
		index := files.Scan(disk.Disk, files.Options{
			Roots:    s.Settings.FileFolders(s.Home),
			Excluded: s.Settings.FileSearchExcludedDirs,
			Packages: disk.Packages,
			Hidden:   disk.Hidden,
		})
		if index.Truncated {
			slog.Warn("The file index is full; index fewer folders", "limit", files.Limit)
		}
		slog.Info("Indexed files", "files", index.Len())
		e.publish(ctx, func(next *search.Snapshot) {
			next.Files = index
			next.FilesReady = true
		})
	}
}

// publish swaps in a rebuilt index and waits until it is published, so the
// slot counts a rebuild as done only once search can see it.
func (e *Engine) publish(ctx context.Context, apply func(next *search.Snapshot)) {
	done := e.Change(func(next *search.Snapshot, _ *store.Store) error {
		apply(next)
		return nil
	})
	select {
	case <-done:
	case <-ctx.Done():
	}
}
