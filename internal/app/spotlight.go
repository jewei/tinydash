package app

import (
	"context"
	"os"
	"sync"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/search"
)

// moreFiles adds the OS file index's results (Spotlight on macOS) to the
// Files tab, after TinyDash's own. The OS search can take a second, so it
// runs on a goroutine for the latest query only; its answer reaches the
// view through done, and the next search shows it. The view calls results
// for every search; the merge lives here, not in package search, because
// search is pure and synchronous, and this is OS work that answers later.
type moreFiles struct {
	// find asks the OS index (platform.FindFiles).
	find func(ctx context.Context, name, folder string, limit int) []string
	// isDir reads the disk: whether path is a folder, and whether it
	// exists, without following a link.
	isDir func(path string) (dir, ok bool)
	// packages and hidden are the index's rules (engine.FileDisk).
	packages []string
	hidden   func(path string) bool
	// remember lets Open and Reveal accept a path shown
	// (engine.Actions.RememberFound).
	remember func(path string)
	// done runs on the goroutine of the OS search once its answer is in.
	done func()

	mu sync.Mutex
	// asked is the name of the last search, and found its answer, nil
	// until it arrives.
	asked  string
	found  []search.FoundFile
	cancel context.CancelFunc
}

// results are the rows the OS index adds to a search of s. A new name
// starts a search; until it answers, there are none.
func (m *moreFiles) results(ctx context.Context, s *search.Snapshot, query string, category core.Category) []core.Result {
	name, ok := search.MoreFilesQuery(s, query, category)
	m.mu.Lock()
	if !ok {
		m.forget()
		m.mu.Unlock()
		return nil
	}
	if name != m.asked {
		m.forget()
		m.asked = name
		var askCtx context.Context
		askCtx, m.cancel = context.WithCancel(ctx)
		go m.ask(askCtx, name, s.Home, s.Settings.FileSearchExcludedDirs)
	}
	found := m.found
	m.mu.Unlock()
	var out []core.Result
	for _, f := range search.MoreFiles(s, name, found) {
		m.remember(f.Path)
		out = append(out, files.FoundResult(f.Path, f.IsDir, s.Home))
	}
	return out
}

// forget drops the last search and stops it; m.mu is held.
func (m *moreFiles) forget() {
	if m.cancel != nil {
		m.cancel()
	}
	m.asked, m.found, m.cancel = "", nil, nil
}

// ask runs the OS search for name under home and keeps what the index's
// rules list, unless a newer search took its place meanwhile.
func (m *moreFiles) ask(ctx context.Context, name, home string, excluded []string) {
	paths := m.find(ctx, name, home, search.MoreFilesLimit)
	found := []search.FoundFile{}
	for _, path := range paths {
		dir, ok := m.isDir(path)
		if ok && files.Lists(path, dir, []string{home}, excluded, m.packages, m.hidden) {
			found = append(found, search.FoundFile{Path: path, IsDir: dir})
		}
	}
	m.mu.Lock()
	current := ctx.Err() == nil && m.asked == name
	if current {
		m.found = found
	}
	m.mu.Unlock()
	if current {
		m.done()
	}
}

// lstatDir is moreFiles.isDir on the real disk.
func lstatDir(path string) (dir, ok bool) {
	info, err := os.Lstat(path)
	return err == nil && info.IsDir(), err == nil
}

func (a *App) newMoreFiles() *moreFiles {
	return &moreFiles{
		find:     platform.FindFiles,
		isDir:    lstatDir,
		packages: platform.PackageExtensions,
		hidden:   platform.HiddenCheck(),
		remember: a.actions.RememberFound,
		done:     func() { a.panel.Update(a.model.Stale) },
	}
}
