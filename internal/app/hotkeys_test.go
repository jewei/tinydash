package app

import (
	"errors"
	"maps"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/search"
)

// fakeRegistry is the OS's shortcut registry: taken keys belong to other
// apps, and ours are the keys TinyDash holds.
type fakeRegistry struct {
	taken map[string]bool
	ours  map[string]func()
}

func newFakeRegistry(taken ...string) *fakeRegistry {
	r := &fakeRegistry{taken: map[string]bool{}, ours: map[string]func(){}}
	for _, k := range taken {
		r.taken[k] = true
	}
	return r
}

func (r *fakeRegistry) Register(accelerator string, fn func()) error {
	if r.taken[accelerator] || r.ours[accelerator] != nil {
		return errors.New("the keys are taken")
	}
	r.ours[accelerator] = fn
	return nil
}

func (r *fakeRegistry) Unregister(accelerator string) { delete(r.ours, accelerator) }

func (r *fakeRegistry) held() []string { return slices.Sorted(maps.Keys(r.ours)) }

// While the hotkey dialog records, the shortcuts are off, and the launcher
// saves the keys it recorded then. Sync must still ask the OS for them, as
// the Rust app's empty registry did, so keys another app holds fail now
// instead of being saved and dead. Checked keys stay off until the pause
// ends.
func TestAHotkeySyncedWhilePausedIsStillCheckedWithTheOS(t *testing.T) {
	os := newFakeRegistry("Alt+N")
	s := &shortcuts{os: os, run: func(core.ID) {}}
	notes, safari := core.ID("app:notes"), core.ID("app:safari")
	if err := s.pause(true); err != nil {
		t.Fatal(err)
	}
	failed := s.sync(search.NewHotkeys(map[core.ID]string{notes: "Alt+N", safari: "Alt+S"}))
	if _, ok := failed[notes]; !ok || len(failed) != 1 {
		t.Fatalf("failed %v, want only %s", failed, notes)
	}
	if !strings.Contains(failed[notes].Error(), "Could not register Alt+N") {
		t.Errorf("error %q", failed[notes])
	}
	if held := os.held(); len(held) != 0 {
		t.Errorf("held %v during the pause", held)
	}
	if err := s.pause(false); err != nil {
		t.Fatal(err)
	}
	if held := os.held(); !slices.Equal(held, []string{"Alt+S"}) {
		t.Errorf("held %v after the pause", held)
	}
}

// Keys that another app took during the pause fail as the pause ends, with
// the same advice as any refused keys.
func TestResumingSaysWhichKeysTheOSRefused(t *testing.T) {
	os := newFakeRegistry()
	s := &shortcuts{os: os, toggle: func() {}, run: func(core.ID) {}}
	if err := s.setLauncher("Control+Shift+Space", "Control+Shift+Space"); err != nil {
		t.Fatal(err)
	}
	if err := s.pause(true); err != nil {
		t.Fatal(err)
	}
	os.taken["Control+Shift+Space"] = true
	err := s.pause(false)
	if err == nil || !strings.HasPrefix(err.Error(), "Could not register Control+Shift+Space: the keys are taken.") || !strings.HasSuffix(err.Error(), shortcutTaken) {
		t.Errorf("error %v", err)
	}
}
