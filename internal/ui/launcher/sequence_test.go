package launcher

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

// The hotkey recorder is checked over every order of its events, not one
// order per bug: rounds of review kept finding orders that spot tests
// missed. After each event the shortcuts are off exactly while a recording
// or a save runs, a recording has nothing to say, and the dialog says what
// its last attempt, a recording or a save, should say:
//   - its own refused save, or else its own refused pause, or else that
//     another save still runs;
//   - else a refused resume since it began;
//   - else nothing.

var (
	errPause  = errors.New("pause refused")
	errResume = errors.New("resume refused")
	errSave   = errors.New("save refused")
)

type recorderEvent int

const (
	evClick   recorderEvent = iota // a click on the recorder: start or stop
	evKeys                         // new keys pressed while recording
	evEscape                       // Escape: stops recording, else closes
	evHide                         // the panel hides
	evSaved                        // the oldest save ends
	evRefused                      // the oldest save is refused
	evReopen                       // the panel shows again, and the dialog opens
	recorderEvents
)

func (e recorderEvent) String() string {
	return [...]string{"click", "keys", "escape", "hide", "saved", "refused", "reopen"}[e]
}

// eachSequence calls fn with every sequence of n events up to length.
func eachSequence(n, length int, fn func([]int)) {
	seq := make([]int, 0, length)
	var walk func()
	walk = func() {
		fn(seq)
		if len(seq) == length {
			return
		}
		for e := range n {
			seq = append(seq, e)
			walk()
			seq = seq[:len(seq)-1]
		}
	}
	walk()
}

// attempt is what the test saw of the dialog's last attempt.
type attempt struct {
	busy         bool // it began while a save ran
	pauseRefused bool
	saveRefused  bool
	resumed      bool
}

func TestTheHotkeyRecorderOverEveryOrderOfEvents(t *testing.T) {
	const length = 6
	runs := 0
	for config := range 4 {
		pauseRefuses, resumeRefuses := config&1 != 0, config&2 != 0
		eachSequence(int(recorderEvents), length, func(seq []int) {
			if len(seq) < length {
				return // each prefix is checked as part of a longer run
			}
			runs++
			if msg := runRecorder(pauseRefuses, resumeRefuses, seq); msg != "" {
				t.Fatalf("pause refuses %v, resume refuses %v: %s", pauseRefuses, resumeRefuses, msg)
			}
		})
	}
	t.Logf("%d runs", runs)
}

func runRecorder(pauseRefuses, resumeRefuses bool, seq []int) string {
	f := newFake(always(hotkeyResult("")))
	f.hold = true
	f.refusePause = func(paused bool) error {
		switch {
		case paused && pauseRefuses:
			return errPause
		case !paused && resumeRefuses:
			return errResume
		}
		return nil
	}
	m := New(f)
	set := core.Do("Set Hotkey…", core.SetHotkey{ID: safari.ID})
	var (
		dialog  *hotkeyDialog
		now     attempt
		current = 0   // the test's number for the last attempt
		saves   []int // the attempt of each save that waits
		trail   []string
	)
	// watch runs an event and notes what the backend did in it.
	watch := func(name string, do func()) {
		trail = append(trail, name)
		calls := len(f.paused)
		runs := len(f.runs)
		before := m.hotkey
		do()
		if m.hotkey != before || m.hotkey != dialog {
			dialog, now = m.hotkey, attempt{}
			current++
		}
		if len(f.runs) > runs {
			current++
			now = attempt{}
			saves = append(saves, current)
		}
		for _, paused := range f.paused[calls:] {
			switch {
			case paused && pauseRefuses:
				now.pauseRefused = true
			case !paused:
				now.resumed = true
			}
		}
	}
	open := func() {
		m.Show("")
		m.start(set, "Safari", "")
	}
	watch("open", open)
	for _, e := range seq {
		ev := recorderEvent(e)
		d := m.hotkey
		switch ev {
		case evClick:
			if d == nil {
				continue
			}
			watch(ev.String(), func() {
				if d.rec.On() {
					m.stopRecording("")
					return
				}
				busy := m.savingHotkey
				m.startRecording()
				if d.rec.On() || now.pauseRefused || busy {
					current++
					now = attempt{busy: busy}
				}
			})
		case evKeys:
			if d == nil || !d.rec.On() {
				continue
			}
			runs := len(f.runs)
			watch(ev.String(), func() { m.stopRecording(fmt.Sprintf("Control+Alt+%d", len(trail))) })
			if len(f.runs) == runs {
				return strings.Join(trail, ", ") + ": the keys pressed were not saved"
			}
		case evEscape:
			if d == nil {
				continue
			}
			watch(ev.String(), func() {
				if d.rec.On() {
					m.stopRecording("")
				} else {
					m.closeHotkey()
				}
			})
		case evHide:
			watch(ev.String(), m.Hidden)
		case evSaved, evRefused:
			if len(f.waiting) == 0 {
				continue
			}
			of := saves[0]
			saves = saves[1:]
			watch(ev.String(), func() {
				if ev == evRefused {
					f.finish(errSave)
				} else {
					f.finish(nil)
				}
			})
			if ev == evRefused && of == current && m.hotkey == dialog {
				now.saveRefused = true
			}
		case evReopen:
			if d != nil {
				continue
			}
			watch(ev.String(), open)
		}
		if msg := checkRecorder(m, f, now, pauseRefuses, resumeRefuses); msg != "" {
			return strings.Join(trail, ", ") + ": " + msg
		}
	}
	return ""
}

func checkRecorder(m *Model, f *fake, now attempt, pauseRefuses, resumeRefuses bool) string {
	// A refused pause leaves the shortcuts on.
	paused := len(f.paused) > 0 && f.paused[len(f.paused)-1] && !pauseRefuses
	recording := m.hotkey != nil && m.hotkey.rec.On()
	if want := recording || m.savingHotkey; paused != want {
		return fmt.Sprintf("shortcuts paused %v, want %v (recording %v, saving %v)", paused, want, recording, m.savingHotkey)
	}
	if m.hotkey == nil {
		return ""
	}
	got := m.hotkey.rec.Err()
	var want string
	switch {
	case recording:
		want = ""
	case now.saveRefused:
		want = errSave.Error()
	case now.pauseRefused:
		want = errPause.Error()
	case now.busy:
		want = errHotkeySaving.Error()
	case now.resumed && resumeRefuses:
		want = errResume.Error()
	}
	if got != want {
		return fmt.Sprintf("dialog says %q, want %q (attempt %+v)", got, want, now)
	}
	return ""
}
