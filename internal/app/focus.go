package app

import (
	"context"
	"errors"
	"log/slog"
	"sync"
	"time"

	"github.com/jewei/tinydash/internal/feature/focus"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// focusMaxWait is the longest wait between clock checks. A wait measures
// elapsed time, which stops while some computers sleep, so a phase that
// ends during a sleep ends this soon after waking.
const focusMaxWait = 30 * time.Second

var errFocusOff = errors.New("The focus timer is off. Turn it on in Settings > Widgets.")

// focusClock runs the focus timer, as src-tauri/src/timer.rs did: run
// waits until the running phase ends, then reports it.
type focusClock struct {
	mu      sync.Mutex
	timer   focus.Timer
	on      bool
	lengths focus.Lengths
	// changed wakes run when the timer changes, so it waits for the new
	// end. It holds one wake at most; more would say nothing new.
	changed chan struct{}
}

func newFocusClock(s settings.Settings) *focusClock {
	l := focusLengths(s)
	return &focusClock{timer: focus.New(l), on: s.ShowFocusTimer, lengths: l, changed: make(chan struct{}, 1)}
}

func focusLengths(s settings.Settings) focus.Lengths {
	return focus.Lengths{
		Focus:      time.Duration(s.FocusMinutes) * time.Minute,
		ShortBreak: time.Duration(s.ShortBreakMinutes) * time.Minute,
		LongBreak:  time.Duration(s.LongBreakMinutes) * time.Minute,
		Sessions:   int(s.SessionsBeforeLongBreak),
	}
}

// follow takes changed settings. Turning the widget off resets the timer,
// so a timer nobody sees never notifies.
func (c *focusClock) follow(s settings.Settings) {
	c.mu.Lock()
	defer c.mu.Unlock()
	l := focusLengths(s)
	switch {
	case c.on && !s.ShowFocusTimer:
		c.timer = focus.New(l)
	case l != c.lengths:
		c.timer.SetLengths(l)
	}
	c.on, c.lengths = s.ShowFocusTimer, l
	c.wake()
}

// control starts, pauses, skips, or resets the timer, as the user asked.
func (c *focusClock) control(control focus.Control, now time.Time) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	if !c.on {
		return errFocusOff
	}
	// A refused control wakes run too: the phase may be up, which only
	// run ends, and a wait that slept with the computer may be far off.
	c.wake()
	return c.timer.Control(control, c.lengths, now)
}

func (c *focusClock) wake() {
	select {
	case c.changed <- struct{}{}:
	default:
	}
}

// widget is the timer as the launcher's pane shows it, or nil while it is
// off. It reads on as control does, so the two never disagree.
func (c *focusClock) widget() *launcher.FocusTimer {
	c.mu.Lock()
	t, on := c.timer, c.on
	c.mu.Unlock()
	if !on {
		return nil
	}
	w := &launcher.FocusTimer{
		Phase: t.Phase.String(), Break: t.Phase != focus.Focus,
		Paused: t.State == focus.Paused, Done: t.State == focus.Done,
		EndsAt: t.EndsAt, Remaining: t.Remaining, Total: t.Total,
		Session: t.Session, Sessions: t.Sessions,
	}
	for _, a := range t.Actions() {
		w.Actions = append(w.Actions, launcher.FocusAction{Label: a.Label, Control: launcher.FocusControl(a.Control)})
	}
	return w
}

// run ends each phase on time and hands it to finished, until ctx ends.
// It calls moved after it pulled in the end of a phase that a wall clock
// set back pushed out. after is time.After, which tests replace.
func (c *focusClock) run(ctx context.Context, now func() time.Time, after func(time.Duration) <-chan time.Time, finished func(focus.Finished), moved func()) {
	for {
		c.mu.Lock()
		// This pass sees every change made so far; a later one wakes the
		// next pass, since it needs the lock.
		select {
		case <-c.changed:
		default:
		}
		at := now()
		pulledIn := c.timer.KeepInPhase(at)
		ended, ok := c.timer.FinishIfDue(c.lengths, at)
		var timeout <-chan time.Time
		if !ok && c.timer.State == focus.Running {
			timeout = after(min(c.timer.Left(at), focusMaxWait))
		}
		c.mu.Unlock()
		if pulledIn {
			moved()
		}
		if ok {
			finished(ended)
			continue
		}
		select {
		case <-ctx.Done():
			return
		case <-c.changed:
		case <-timeout:
		}
	}
}

// focusFinished tells the user that a phase ended, in a system
// notification, and shows the next phase in the pane. Each notification
// has an ID of its own, so it never replaces the last one; on macOS the
// first one asks to allow them.
func (a *App) focusFinished(ended focus.Finished) {
	go func() {
		if err := showNotification(ended.Title, ended.Body); err != nil {
			slog.Warn("Could not show the focus timer notification", "error", err)
		}
	}()
	a.panel.Update(a.model.WidgetsChanged)
}

func (b *backend) FocusTimer() *launcher.FocusTimer { return b.app().focus.widget() }

func (b *backend) ControlFocus(control launcher.FocusControl) (*launcher.FocusTimer, error) {
	a := b.app()
	err := a.focus.control(focus.Control(control), time.Now())
	return a.focus.widget(), err
}
