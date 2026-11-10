package settings

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/settings"
)

// The launcher shortcut's recorder is checked over every order of its
// events and replies, not one order per bug: rounds of review kept
// finding orders that spot tests missed. A recording has nothing to say.
// Once every reply is in, the shortcut is paused exactly while recording,
// and the page says what the last recording should say:
//   - its own refused save, or else its own refused pause;
//   - else a refused resume after it;
//   - else nothing.

var (
	errPause  = errors.New("pause refused")
	errResume = errors.New("resume refused")
	errSave   = errors.New("save refused")
)

const (
	evClick = iota // a click on the recorder: start or stop
	evKeys         // new keys pressed while recording
	evReply        // the oldest reply comes
	evClose        // the window closes
	recorderEvents
)

var eventNames = [...]string{"click", "keys", "reply", "close"}

// eachSequence calls fn with every sequence of n events of length.
func eachSequence(n, length int, fn func([]int)) {
	seq := make([]int, length)
	var walk func(int)
	walk = func(i int) {
		if i == length {
			fn(seq)
			return
		}
		for e := range n {
			seq[i] = e
			walk(i + 1)
		}
	}
	walk(0)
}

// recorded is what the test saw of the last recording.
type recorded struct {
	started, stopped, saved bool
}

func TestTheShortcutRecorderOverEveryOrderOfEvents(t *testing.T) {
	const length = 8
	runs := 0
	for config := range 8 {
		pauseRefuses, resumeRefuses, saveRefuses := config&1 != 0, config&2 != 0, config&4 != 0
		eachSequence(recorderEvents, length, func(seq []int) {
			runs++
			if msg := runRecorder(pauseRefuses, resumeRefuses, saveRefuses, seq); msg != "" {
				t.Fatalf("pause refuses %v, resume refuses %v, save refuses %v: %s", pauseRefuses, resumeRefuses, saveRefuses, msg)
			}
		})
	}
	t.Logf("%d runs", runs)
}

func runRecorder(pauseRefuses, resumeRefuses, saveRefuses bool, seq []int) string {
	f := newFake()
	f.hold = true
	f.pauseErr = func(paused bool) error {
		switch {
		case paused && pauseRefuses:
			return errPause
		case !paused && resumeRefuses:
			return errResume
		}
		return nil
	}
	if saveRefuses {
		f.refuse = func(settings.Settings) error { return errSave }
	}
	m := New(f)
	var (
		last  recorded
		trail []string
	)
	check := func() string {
		if m.rec.On() && m.rec.Err() != "" {
			return fmt.Sprintf("recording says %q", m.rec.Err())
		}
		return ""
	}
	for _, e := range seq {
		trail = append(trail, eventNames[e])
		switch e {
		case evClick:
			if m.rec.On() {
				m.stopRecording("")
				last.stopped = true
			} else {
				m.startRecording()
				last = recorded{started: true}
			}
		case evKeys:
			if m.rec.On() {
				keys := "Alt+Space"
				if m.values().Shortcut == keys {
					keys = "Alt+K"
				}
				m.stopRecording(keys)
				last.stopped, last.saved = true, true
			}
		case evReply:
			if len(f.waiting) > 0 {
				done := f.waiting[0]
				f.waiting = f.waiting[1:]
				done()
			}
		case evClose:
			if m.rec.On() {
				last.stopped = true
			}
			m.Close()
		}
		if msg := check(); msg != "" {
			return strings.Join(trail, ", ") + ": " + msg
		}
	}
	// Every reply comes.
	for len(f.waiting) > 0 {
		done := f.waiting[0]
		f.waiting = f.waiting[1:]
		done()
	}
	trail = append(trail, "every reply")
	if msg := check(); msg != "" {
		return strings.Join(trail, ", ") + ": " + msg
	}
	paused := false
	for _, p := range f.pauses {
		paused = p && !pauseRefuses
	}
	if paused != m.rec.On() {
		return fmt.Sprintf("%s: shortcut paused %v, recording %v", strings.Join(trail, ", "), paused, m.rec.On())
	}
	var want string
	switch {
	case !last.started:
	case last.saved && saveRefuses:
		want = errSave.Error()
	case pauseRefuses:
		want = errPause.Error()
	case last.stopped && resumeRefuses:
		want = errResume.Error()
	}
	if got := m.rec.Err(); got != want {
		return fmt.Sprintf("%s: says %q, want %q (last %+v)", strings.Join(trail, ", "), got, want, last)
	}
	return ""
}
