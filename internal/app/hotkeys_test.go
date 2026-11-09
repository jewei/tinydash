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
	// The engine saves no refused hotkey: it syncs back without it.
	s.sync(search.NewHotkeys(map[core.ID]string{safari: "Alt+S"}))
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

// Keys that the OS refused, at startup or as a pause ends, stay saved. When
// the other app lets them go, the next resume registers them again, as the
// Rust registry did: re-recording the same keys in Settings only resumes,
// because nothing changed.
func TestRefusedKeysAreTriedAgainByTheNextResume(t *testing.T) {
	for _, at := range []string{"startup", "resume"} {
		os := newFakeRegistry()
		s := &shortcuts{os: os, toggle: func() {}, run: func(core.ID) {}}
		notes := core.ID("app:notes")
		wanted := search.NewHotkeys(map[core.ID]string{notes: "Alt+N"})
		if at == "startup" {
			os.taken["Alt+N"], os.taken["Control+Shift+Space"] = true, true
		}
		s.sync(wanted)
		s.setLauncher("Control+Shift+Space", "Control+Shift+Space")
		if at == "resume" {
			s.pause(true)
			os.taken["Alt+N"], os.taken["Control+Shift+Space"] = true, true
			if err := s.pause(false); err == nil {
				t.Fatal("resuming with taken keys gave no error")
			}
		}
		if held := os.held(); len(held) != 0 {
			t.Fatalf("%s: held %v while the keys were taken", at, held)
		}
		delete(os.taken, "Alt+N")
		delete(os.taken, "Control+Shift+Space")
		s.pause(true)
		if err := s.pause(false); err != nil {
			t.Fatalf("%s: resuming with free keys: %v", at, err)
		}
		if held := os.held(); !slices.Equal(held, []string{"Alt+N", "Control+Shift+Space"}) {
			t.Errorf("%s: held %v, want the hotkey and the launcher shortcut", at, held)
		}
	}
}

// A sync with the same hotkeys tries a refused one again.
func TestRefusedHotkeysAreTriedAgainByTheNextSync(t *testing.T) {
	os := newFakeRegistry("Alt+N")
	s := &shortcuts{os: os, run: func(core.ID) {}}
	wanted := search.NewHotkeys(map[core.ID]string{"app:notes": "Alt+N"})
	if failed := s.sync(wanted); len(failed) != 1 {
		t.Fatalf("failed %v, want Alt+N", failed)
	}
	delete(os.taken, "Alt+N")
	if failed := s.sync(wanted); len(failed) != 0 {
		t.Fatalf("failed %v once the keys were free", failed)
	}
	if held := os.held(); !slices.Equal(held, []string{"Alt+N"}) {
		t.Errorf("held %v", held)
	}
}
