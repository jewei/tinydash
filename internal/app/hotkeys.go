package app

import (
	"fmt"
	"log/slog"
	"strings"

	"github.com/egoist/mygo"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
)

// shortcuts holds the launcher shortcut and the per-result hotkeys
// registered with the OS. Shortcuts are stored in the Rust app's Tauri
// syntax and turned into mygo accelerators only to register them.
//
// Its state lives on the main thread: each method runs there, as mygo's
// shortcut calls do anyway. A lock instead could deadlock: the writer
// would hold it while mygo waits on the main thread, and the main thread
// would wait on it to pause the shortcuts for the hotkey recorder.
type shortcuts struct {
	// os registers keys with the OS: mygo.GlobalShortcut, or a fake in tests.
	os       registry
	launcher string // the saved launcher shortcut as a mygo accelerator, "" when none
	toggle   func()
	hotkeys  map[core.ID]string // saved hotkeys: result ID -> mygo accelerator
	run      func(core.ID)
	paused   bool
	// held is the accelerators that the OS accepted and holds for us. The
	// saved keys stay saved when the OS refuses them, so the next resume,
	// sync, or save tries them again, as in the Rust registry.
	held map[string]bool
}

// registry is the part of mygo.GlobalShortcut that shortcuts uses.
type registry interface {
	Register(accelerator string, fn func()) error
	Unregister(accelerator string)
}

var modifierBits = map[string]engine.Modifiers{
	"control": engine.ModControl, "ctrl": engine.ModControl,
	"alt": engine.ModAlt, "option": engine.ModAlt,
	"shift": engine.ModShift,
	"super": engine.ModSuper, "command": engine.ModSuper, "cmd": engine.ModSuper, "meta": engine.ModSuper,
}

// Parse reads a stored shortcut, so the engine can compare two shortcuts
// and check that one can be a hotkey.
func (s *shortcuts) Parse(stored string) (engine.Keys, error) {
	accelerator, err := settings.Accelerator(stored)
	if err != nil {
		return engine.Keys{}, err
	}
	// The key comes from the accelerator, so two spellings of one key, as
	// "Super+KeyK" and "Super+K", compare equal.
	parts := strings.Split(accelerator, "+")
	var keys engine.Keys
	for _, part := range parts[:len(parts)-1] {
		keys.Mods |= modifierBits[strings.ToLower(part)]
	}
	keys.Key = parts[len(parts)-1]
	if len(keys.Key) == 1 {
		keys.Key = strings.ToUpper(keys.Key)
	}
	return keys, nil
}

// Sync makes the registered hotkeys match wanted.
func (s *shortcuts) Sync(wanted search.Hotkeys) (failed map[core.ID]error) {
	mygo.RunOnMain(func() { failed = s.sync(wanted) })
	return failed
}

func (s *shortcuts) sync(wanted search.Hotkeys) map[core.ID]error {
	next := map[core.ID]string{}
	failed := map[core.ID]error{}
	for _, pair := range wanted.All() {
		accelerator, err := settings.Accelerator(pair.Value)
		if err != nil {
			failed[pair.ID] = err
			continue
		}
		next[pair.ID] = accelerator
	}
	for id, accelerator := range s.hotkeys {
		if next[id] != accelerator {
			s.release(accelerator)
		}
	}
	s.hotkeys = next
	for id, accelerator := range next {
		if s.held[accelerator] {
			continue
		}
		if err := s.take(accelerator, func() { go s.run(id) }); err != nil {
			failed[id] = err
		}
	}
	return failed
}

// take registers keys with the OS. While the shortcuts pause, the launcher
// and Settings save keys that they recorded: registering checks that
// another app does not hold them, and they stay off until the pause ends.
func (s *shortcuts) take(accelerator string, fn func()) error {
	if err := s.os.Register(accelerator, fn); err != nil {
		return registerError(accelerator, err)
	}
	if s.paused {
		s.os.Unregister(accelerator)
		return nil
	}
	if s.held == nil {
		s.held = map[string]bool{}
	}
	s.held[accelerator] = true
	return nil
}

// release gives keys back to the OS if it holds them for us.
func (s *shortcuts) release(accelerator string) {
	if s.held[accelerator] {
		s.os.Unregister(accelerator)
		delete(s.held, accelerator)
	}
}

// registerError says that the OS refused keys, and what to do.
func registerError(keys string, err error) error {
	return fmt.Errorf("Could not register %s: %v.%s", keys, err, shortcutTaken)
}

// SetLauncher registers the launcher shortcut, stored in Tauri syntax.
func (s *shortcuts) SetLauncher(stored string) error {
	accelerator, err := settings.Accelerator(stored)
	if err != nil {
		return err
	}
	mygo.RunOnMain(func() { err = s.setLauncher(accelerator) })
	return err
}

func (s *shortcuts) setLauncher(accelerator string) error {
	s.release(s.launcher)
	s.launcher = accelerator
	return s.take(accelerator, s.toggle)
}

// Pause turns every shortcut off while the hotkey dialog records keys, so a
// press records instead of running, and on again after.
func (s *shortcuts) Pause(paused bool) (err error) {
	mygo.RunOnMain(func() { err = s.pause(paused) })
	return err
}

func (s *shortcuts) pause(paused bool) error {
	if paused == s.paused {
		return nil
	}
	if paused {
		for accelerator := range s.held {
			s.release(accelerator)
		}
		s.paused = true
		return nil
	}
	s.paused = false
	// Register every saved key again. Keys that another app holds now are
	// refused and stay saved, so the next resume tries them again. As in
	// the Rust app, only the launcher shortcut's error is returned: the
	// recorder that just ran is not about another result's hotkey, which
	// is only logged.
	var err error
	if s.launcher != "" {
		err = s.take(s.launcher, s.toggle)
	}
	for id, accelerator := range s.hotkeys {
		if refused := s.take(accelerator, func() { go s.run(id) }); refused != nil {
			slog.Warn("A hotkey is off", "id", id, "error", refused)
		}
	}
	return err
}
