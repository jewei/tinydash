package app

import (
	"fmt"
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
	launcher string // mygo accelerator, "" when none
	toggle   func()
	hotkeys  map[core.ID]string // result ID -> mygo accelerator
	run      func(core.ID)
	paused   bool
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
	if _, err := settings.Accelerator(stored); err != nil {
		return engine.Keys{}, err
	}
	parts := strings.Split(stored, "+")
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
		if next[id] != accelerator && !s.paused {
			mygo.GlobalShortcut.Unregister(accelerator)
		}
	}
	for id, accelerator := range next {
		if s.hotkeys[id] == accelerator || s.paused {
			continue
		}
		if err := s.register(id, accelerator); err != nil {
			failed[id] = err
			delete(next, id)
		}
	}
	s.hotkeys = next
	return failed
}

func (s *shortcuts) register(id core.ID, accelerator string) error {
	if err := mygo.GlobalShortcut.Register(accelerator, func() { go s.run(id) }); err != nil {
		return fmt.Errorf("%s is taken by another app or by the system: %v", accelerator, err)
	}
	return nil
}

// SetLauncher registers the launcher shortcut, stored in Tauri syntax.
func (s *shortcuts) SetLauncher(stored string) error {
	accelerator, err := settings.Accelerator(stored)
	if err != nil {
		return err
	}
	mygo.RunOnMain(func() { err = s.setLauncher(stored, accelerator) })
	return err
}

func (s *shortcuts) setLauncher(stored, accelerator string) error {
	if s.launcher != "" && !s.paused {
		mygo.GlobalShortcut.Unregister(s.launcher)
	}
	s.launcher = ""
	if err := mygo.GlobalShortcut.Register(accelerator, s.toggle); err != nil {
		return fmt.Errorf("TinyDash could not use %s to open the launcher: %v. Choose another shortcut in Settings.", stored, err)
	}
	// Settings saves a new shortcut while the shortcuts pause to record
	// it. Registering checked that another app does not hold the keys;
	// they stay off until the pause ends.
	if s.paused {
		mygo.GlobalShortcut.Unregister(accelerator)
	}
	s.launcher = accelerator
	return nil
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
	s.paused = paused
	all := map[string]func(){}
	if s.launcher != "" {
		all[s.launcher] = s.toggle
	}
	for id, accelerator := range s.hotkeys {
		all[accelerator] = func() { go s.run(id) }
	}
	var first error
	for accelerator, fn := range all {
		if paused {
			mygo.GlobalShortcut.Unregister(accelerator)
		} else if err := mygo.GlobalShortcut.Register(accelerator, fn); err != nil && first == nil {
			first = err
		}
	}
	return first
}
