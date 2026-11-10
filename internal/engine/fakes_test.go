package engine

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"math/rand/v2"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

// steps records, in order, what actions asked of the OS and the shell.
type steps struct {
	mu   sync.Mutex
	list []string
}

func (s *steps) add(step string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.list = append(s.list, step)
}

// take returns the steps so far and forgets them.
func (s *steps) take() []string {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := s.list
	s.list = nil
	return out
}

// fakeOS records each call. fail makes the calls that act fail.
type fakeOS struct {
	*steps
	fail error
	// clipboard is the clipboard's text; secret marks it as a password
	// manager's copy.
	clipboard string
	secret    bool
}

func (f *fakeOS) act(step string) error {
	f.add(step)
	return f.fail
}

func (f *fakeOS) LaunchApp(path string) error { return f.act("launch " + golden.Slash(path)) }
func (f *fakeOS) OpenPath(path string) error  { return f.act("open " + golden.Slash(path)) }
func (f *fakeOS) Reveal(path string) error    { return f.act("reveal " + golden.Slash(path)) }
func (f *fakeOS) OpenURL(url string) error    { return f.act("open " + url) }
func (f *fakeOS) WriteText(text string, secret bool) error {
	if secret {
		return f.act("copy secret " + text)
	}
	return f.act("copy " + text)
}
func (f *fakeOS) WriteClip(c clipboard.Content) error {
	return f.act(fmt.Sprintf("copy %s %q", c.Kind, c.Text+strings.Join(c.Files, ",")))
}
func (f *fakeOS) ClipboardText(secretToo bool) (string, bool) {
	f.add("read clipboard")
	if f.clipboard == "" || f.secret && !secretToo {
		return "", false
	}
	return f.clipboard, true
}
func (f *fakeOS) RunSystem(c core.SystemCommand) error { return f.act("run " + string(c)) }
func (f *fakeOS) OpenCalendarApp(kind core.UpNextKind) error {
	return f.act("open up next " + string(kind))
}

type fakeShell struct{ *steps }

func (f fakeShell) Hide() error                { f.add("hide"); return nil }
func (f fakeShell) Dismiss() error             { f.add("dismiss"); return nil }
func (f fakeShell) ShowAgain() error           { f.add("show again"); return nil }
func (f fakeShell) ShowHUD(text string)        { f.add("hud " + text) }
func (f fakeShell) OpenSettings() error        { f.add("settings"); return nil }
func (f fakeShell) CenterLauncher() error      { f.add("center"); return nil }
func (f fakeShell) TypeQuery(text string)      { f.add("type " + text) }
func (f fakeShell) Refresh()                   { f.add("refresh") }
func (f fakeShell) TurnOnCurrencyRates() error { f.add("rates on"); return nil }
func (f fakeShell) Quit()                      { f.add("quit") }
func (f fakeShell) AskCalendarAccess()         { f.add("ask calendar access") }

// fakeKeys reads the accelerators the tests use as Tauri does: names in
// any case, and a letter with or without its Key prefix. failing refuses
// to register these result IDs.
type fakeKeys struct {
	mu      sync.Mutex
	synced  []search.Hotkeys
	failing map[core.ID]error
}

func (f *fakeKeys) Parse(accelerator string) (Keys, error) {
	parts := strings.Split(accelerator, "+")
	var keys Keys
	for _, part := range parts[:len(parts)-1] {
		switch strings.ToLower(part) {
		case "shift":
			keys.Mods |= ModShift
		case "control", "ctrl":
			keys.Mods |= ModControl
		case "alt", "option":
			keys.Mods |= ModAlt
		case "super", "command", "cmd":
			keys.Mods |= ModSuper
		default:
			return Keys{}, fmt.Errorf("“%s” is not a valid shortcut: unknown modifier.", accelerator)
		}
	}
	key := strings.ToLower(parts[len(parts)-1])
	key = strings.TrimPrefix(key, "key")
	switch {
	case len(key) == 1 && key[0] >= 'a' && key[0] <= 'z':
		keys.Key = "Key" + strings.ToUpper(key)
	case key == "space":
		keys.Key = "Space"
	case strings.HasPrefix(key, "f") && isNumber(key[1:]):
		keys.Key = "F" + key[1:]
	default:
		return Keys{}, fmt.Errorf("“%s” is not a valid shortcut: unknown key.", accelerator)
	}
	return keys, nil
}

func isNumber(s string) bool {
	_, err := strconv.Atoi(s)
	return err == nil
}

func (f *fakeKeys) Sync(wanted search.Hotkeys) map[core.ID]error {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.synced = append(f.synced, wanted)
	return f.failing
}

// osDisk reads the real disk for scans of temporary folders.
type osDisk struct{}

func (osDisk) ReadDir(dir string) ([]fs.DirEntry, error) { return os.ReadDir(dir) }
func (osDisk) RealDir(dir string) (string, bool) {
	real, err := filepath.EvalSymlinks(dir)
	if err != nil {
		return dir, false
	}
	info, err := os.Stat(real)
	return real, err == nil && info.IsDir()
}

var testNow = time.Date(2026, 11, 9, 10, 30, 0, 0, time.UTC)

var (
	home   = golden.Abs("/Users/me")
	safari = golden.Abs("/Applications/Safari.app")
)

// world is a running engine with its actions over an in-memory store,
// Safari installed, and both scans done.
type world struct {
	t       *testing.T
	engine  *Engine
	actions *Actions
	host    *fakeOS
	keys    *fakeKeys
	steps   *steps
	db      *store.Store
}

// newWorld starts a world; setup may change the first snapshot and the
// store before the engine starts.
func newWorld(t *testing.T, setup func(first *search.Snapshot, db *store.Store)) *world {
	t.Helper()
	ctx := context.Background()
	db, err := store.OpenMemory(ctx)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	first, warnings := Load(ctx, db, settings.Default(), home, rand.NewChaCha8([32]byte{1}))
	if len(warnings) > 0 {
		t.Fatal(warnings)
	}
	first.Apps = apps.NewIndex([]apps.App{{Name: "Safari", Path: safari}})
	first.AppsReady, first.FilesReady = true, true
	if setup != nil {
		setup(&first, db)
	}
	run, stop := context.WithCancel(ctx)
	t.Cleanup(stop)
	e := New(db, first, nil)
	go e.Run(run)
	log := &steps{}
	w := &world{t: t, engine: e, host: &fakeOS{steps: log}, keys: &fakeKeys{}, steps: log, db: db}
	w.actions = NewActions(e, w.host, fakeShell{log}, w.keys, func() time.Time { return testNow })
	return w
}

func (w *world) run(action core.Action, id core.ID) error {
	return w.actions.Run(FromLauncher, action, id)
}

func (w *world) snapshot() *search.Snapshot { return w.engine.Snapshot(testNow) }

// edit changes the published state, as a change from elsewhere would.
func (w *world) edit(change func(next *search.Snapshot)) {
	w.t.Helper()
	err := <-w.engine.Change(func(next *search.Snapshot, _ *store.Store) error {
		change(next)
		return nil
	})
	if err != nil {
		w.t.Fatal(err)
	}
}

func errorText(err error) string {
	if err == nil {
		return ""
	}
	return err.Error()
}

func wantSteps(t *testing.T, w *world, want ...string) {
	t.Helper()
	if got := w.steps.take(); !slices.Equal(got, want) {
		t.Errorf("steps %q, want %q", got, want)
	}
}

var errNoDarkMode = errors.New("This desktop has no dark mode.")
