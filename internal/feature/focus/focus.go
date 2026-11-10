// Package focus is the focus timer: Pomodoro sessions with short breaks,
// and a long break after the last session of a cycle. It is a port of
// src-tauri/src/features/focus.rs. The caller passes the time in, so the
// logic never waits and tests need no clock.
package focus

import (
	"errors"
	"fmt"
	"time"
)

// Phase is what the timer counts down.
type Phase uint8

const (
	Focus Phase = iota
	ShortBreak
	LongBreak
)

// String is the phase's name as the widget shows it.
func (p Phase) String() string {
	switch p {
	case ShortBreak:
		return "Short break"
	case LongBreak:
		return "Long break"
	}
	return "Focus"
}

// State is whether the timer runs.
type State uint8

const (
	// Idle is ready to start the phase at its full length.
	Idle State = iota
	Running
	Paused
	// Done: a focus session ended, and its break waits for the user.
	Done
)

// Control is what the user asks of the timer. Each is checked against the
// state.
type Control uint8

const (
	// Start starts focus, resumes after a pause, or starts the break that
	// is due.
	Start Control = iota
	Pause
	// SkipBreak ends a break, or skips the break that is due, and gets
	// ready for the next session.
	SkipBreak
	// Reset starts the cycle over: session 1, not running.
	Reset
)

// ErrChanged is why a control does not fit the state, as when the phase
// ended between the click and the control.
var ErrChanged = errors.New("The focus timer changed. Try again.")

// Lengths are the phase lengths and the sessions in a cycle, from the
// settings.
type Lengths struct {
	Focus, ShortBreak, LongBreak time.Duration
	Sessions                     int
}

// Finished is a phase that ended on its own, for the notification.
type Finished struct {
	Title, Body string
}

// Action is a control the user can choose now, with its label.
type Action struct {
	Label   string
	Control Control
}

// Timer is the focus timer's state. The zero value is not ready: start
// from New.
type Timer struct {
	Phase Phase
	State State
	// Remaining is the time left while the timer does not run.
	Remaining time.Duration
	// EndsAt is when the running phase ends, or zero while it does not
	// run. It holds no monotonic reading, so the time left follows the
	// wall clock, also across a sleep of the computer.
	EndsAt time.Time
	// Total is the length of the phase, for the progress ring.
	Total time.Duration
	// Session is the session of the cycle, from 1, of Sessions.
	Session, Sessions int
}

// New is a timer ready for the first focus session of a cycle.
func New(l Lengths) Timer {
	t := Timer{Session: 1, Sessions: l.Sessions}
	t.ready(Focus, l)
	return t
}

// SetLengths takes new lengths from the settings. A phase on its way keeps
// its length; the new ones count from the next phase.
func (t *Timer) SetLengths(l Lengths) {
	t.Sessions = l.Sessions
	t.Session = min(t.Session, l.Sessions)
	// An idle timer is always at the start of a focus session.
	if t.State == Idle {
		t.ready(Focus, l)
	}
}

// Control does what the user asked at now, or returns ErrChanged when it
// does not fit the state.
func (t *Timer) Control(c Control, l Lengths, now time.Time) error {
	// A phase whose time is up waits for FinishIfDue, which reports it.
	// Pausing or skipping it now would act on a phase that is over.
	if c != Reset && t.State == Running && t.Left(now) == 0 {
		return ErrChanged
	}
	switch {
	case c == Start && (t.State == Idle || t.State == Paused):
		t.run(now)
	case c == Start && t.State == Done:
		phase := ShortBreak
		if t.Session >= t.Sessions {
			phase = LongBreak
		}
		t.ready(phase, l)
		t.run(now)
	case c == Pause && t.State == Running:
		t.Remaining = t.Left(now)
		t.State = Paused
		t.EndsAt = time.Time{}
	case c == SkipBreak && (t.onBreak() || t.State == Done):
		t.nextSession(l)
	case c == Reset:
		*t = New(l)
	default:
		return ErrChanged
	}
	return nil
}

// FinishIfDue ends the phase if its time is up at now: a focus session
// waits for its break, and a break readies the next session. It reports
// the phase that ended.
func (t *Timer) FinishIfDue(l Lengths, now time.Time) (Finished, bool) {
	if t.State != Running || t.Left(now) > 0 {
		return Finished{}, false
	}
	if t.Phase == Focus {
		t.State = Done
		t.Remaining = 0
		t.EndsAt = time.Time{}
		length, kind := l.ShortBreak, "break"
		if t.Session >= t.Sessions {
			length, kind = l.LongBreak, "long break"
		}
		return Finished{
			Title: "Focus session done",
			Body:  fmt.Sprintf("Time for a %d-minute %s.", int(length/time.Minute), kind),
		}, true
	}
	t.nextSession(l)
	return Finished{
		Title: "Break is over",
		Body:  fmt.Sprintf("Ready for focus session %d of %d?", t.Session, t.Sessions),
	}, true
}

// Left is the time left in the phase at now, never more than the phase,
// also after a wall clock was set back. KeepInPhase then pulls the end in.
func (t *Timer) Left(now time.Time) time.Duration {
	if t.EndsAt.IsZero() {
		return t.Remaining
	}
	return min(max(t.EndsAt.Sub(now), 0), t.Total)
}

// KeepInPhase pulls in the end of a running phase that a wall clock set
// back pushed further away than the phase is long, so the phase counts
// down again and ends at most its length after now. It reports whether
// it moved the end.
func (t *Timer) KeepInPhase(now time.Time) bool {
	if t.State != Running || t.EndsAt.Sub(now) <= t.Total {
		return false
	}
	t.EndsAt = now.Round(0).Add(t.Total)
	return true
}

// Actions are what the user can do now; the first is the main one.
func (t *Timer) Actions() []Action {
	var actions []Action
	switch t.State {
	case Idle:
		actions = append(actions, Action{"Start Focus", Start})
	case Running:
		actions = append(actions, Action{"Pause Timer", Pause})
	case Paused:
		actions = append(actions, Action{"Resume Timer", Start})
	case Done:
		actions = append(actions, Action{"Start Break", Start})
	}
	if t.onBreak() || t.State == Done {
		actions = append(actions, Action{"Skip Break", SkipBreak})
	}
	if fresh := t.State == Idle && t.Session == 1; !fresh {
		actions = append(actions, Action{"Reset Timer", Reset})
	}
	return actions
}

func (t *Timer) onBreak() bool { return t.Phase != Focus }

// run starts the countdown of the time left at now. Round(0) drops the
// monotonic reading, which stops while the computer sleeps on some
// systems.
func (t *Timer) run(now time.Time) {
	t.State = Running
	t.EndsAt = now.Round(0).Add(t.Remaining)
}

func (t *Timer) nextSession(l Lengths) {
	if t.Phase == LongBreak || t.Session >= t.Sessions {
		t.Session = 1
	} else {
		t.Session++
	}
	t.ready(Focus, l)
}

// ready stops at the start of phase, at its full length.
func (t *Timer) ready(phase Phase, l Lengths) {
	t.Phase = phase
	t.State = Idle
	switch phase {
	case ShortBreak:
		t.Total = l.ShortBreak
	case LongBreak:
		t.Total = l.LongBreak
	default:
		t.Total = l.Focus
	}
	t.Remaining = t.Total
	t.EndsAt = time.Time{}
}
