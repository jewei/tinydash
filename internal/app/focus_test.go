package app

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/feature/focus"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

func focusOn() settings.Settings {
	s := settings.Default()
	s.ShowFocusTimer = true
	return s
}

func TestTurningTheFocusTimerOffResetsARunningTimer(t *testing.T) {
	on := focusOn()
	clock := newFocusClock(on)
	start := time.Date(2026, 11, 10, 9, 0, 0, 0, time.UTC)
	if err := clock.control(focus.Start, start); err != nil {
		t.Fatal(err)
	}

	longer := on
	longer.FocusMinutes = 50
	clock.follow(longer)
	if clock.timer.State != focus.Running || clock.timer.Total != 25*time.Minute {
		t.Errorf("new lengths changed the running phase: %+v", clock.timer)
	}

	off := longer
	off.ShowFocusTimer = false
	clock.follow(off)
	if clock.timer != focus.New(focusLengths(off)) {
		t.Errorf("%+v", clock.timer)
	}
	if err := clock.control(focus.Start, start); !errors.Is(err, errFocusOff) {
		t.Errorf("an off timer took a control: %v", err)
	}
}

func TestTheFocusTimerWidgetShowsTheTimer(t *testing.T) {
	clock := newFocusClock(focusOn())
	start := time.Date(2026, 11, 10, 9, 0, 0, 0, time.UTC)
	if err := clock.control(focus.Start, start); err != nil {
		t.Fatal(err)
	}
	w := clock.widget(focusOn())
	if w.Phase != "Focus" || w.Break || w.Paused || w.Done || !w.EndsAt.Equal(start.Add(25*time.Minute)) ||
		w.Total != 25*time.Minute || w.Session != 1 || w.Sessions != 4 || len(w.Actions) != 2 ||
		w.Actions[0] != (launcher.FocusAction{Label: "Pause Timer", Control: launcher.FocusControl(focus.Pause)}) {
		t.Errorf("%+v", w)
	}
	// The pane hands the control back as it came.
	if err := clock.control(focus.Control(w.Actions[0].Control), start.Add(time.Minute)); err != nil || clock.timer.State != focus.Paused {
		t.Errorf("%+v %v", clock.timer, err)
	}
}

func TestTheFocusTimerWidgetIsGoneWhileItIsOff(t *testing.T) {
	clock := newFocusClock(focusOn())
	if w := clock.widget(settings.Default()); w != nil {
		t.Errorf("an off timer shows: %+v", w)
	}
}

// The real clock's end is a wall clock time, so a sleep of the computer
// does not hold it back.
func TestTheFocusTimerEndFollowsTheWallClock(t *testing.T) {
	clock := newFocusClock(focusOn())
	if err := clock.control(focus.Start, time.Now()); err != nil {
		t.Fatal(err)
	}
	if end := clock.widget(focusOn()).EndsAt; end != end.Round(0) {
		t.Errorf("the end %v holds a monotonic reading", end)
	}
}

// fakeTime is a clock that a test moves, with waits it ends.
type fakeTime struct {
	mu    sync.Mutex
	now   time.Time
	waits chan time.Duration
	fire  chan time.Time
}

func (f *fakeTime) Now() time.Time {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.now
}

func (f *fakeTime) set(now time.Time) {
	f.mu.Lock()
	f.now = now
	f.mu.Unlock()
}

func (f *fakeTime) After(d time.Duration) <-chan time.Time {
	f.waits <- d
	return f.fire
}

func TestTheFocusClockReportsAFocusEndAndThenABreakEnd(t *testing.T) {
	s := focusOn()
	s.FocusMinutes, s.ShortBreakMinutes = 1, 1
	clock := newFocusClock(s)
	start := time.Date(2026, 11, 10, 9, 0, 0, 0, time.UTC)
	clocks := &fakeTime{now: start, waits: make(chan time.Duration, 8), fire: make(chan time.Time)}
	finished := make(chan focus.Finished, 2)
	ctx, cancel := context.WithCancel(context.Background())
	stopped := make(chan struct{})
	go func() {
		clock.run(ctx, clocks.Now, clocks.After, func(f focus.Finished) { finished <- f })
		close(stopped)
	}()
	defer func() {
		cancel()
		<-stopped
	}()
	wait := func() time.Duration {
		t.Helper()
		select {
		case d := <-clocks.waits:
			return d
		case <-time.After(5 * time.Second):
			t.Fatal("the clock never waited")
		}
		return 0
	}
	ended := func() focus.Finished {
		t.Helper()
		select {
		case f := <-finished:
			return f
		case <-time.After(5 * time.Second):
			t.Fatal("no phase ended")
		}
		return focus.Finished{}
	}

	if err := clock.control(focus.Start, start); err != nil {
		t.Fatal(err)
	}
	if d := wait(); d != 30*time.Second {
		t.Errorf("waited %v, want at most focusMaxWait", d)
	}
	clocks.set(start.Add(40 * time.Second))
	clocks.fire <- time.Time{}
	if d := wait(); d != 20*time.Second {
		t.Errorf("waited %v for the last 20 seconds", d)
	}
	clocks.set(start.Add(time.Minute))
	clocks.fire <- time.Time{}
	if f := ended(); f.Title != "Focus session done" || f.Body != "Time for a 1-minute break." {
		t.Errorf("%+v", f)
	}

	if err := clock.control(focus.Start, start.Add(70*time.Second)); err != nil {
		t.Fatal(err)
	}
	if d := wait(); d != 30*time.Second {
		t.Errorf("waited %v", d)
	}
	clocks.set(start.Add(130 * time.Second))
	clocks.fire <- time.Time{}
	if f := ended(); f.Title != "Break is over" || f.Body != "Ready for focus session 2 of 4?" {
		t.Errorf("%+v", f)
	}

	// Session 2 ends, and its break runs while the computer sleeps
	// through its end. A pause after waking is refused, and wakes the
	// clock at once instead of after the wait that slept, so the break
	// ends now.
	if err := clock.control(focus.Start, start.Add(3*time.Minute)); err != nil {
		t.Fatal(err)
	}
	wait()
	clocks.set(start.Add(4 * time.Minute))
	clocks.fire <- time.Time{}
	ended()
	if err := clock.control(focus.Start, start.Add(4*time.Minute)); err != nil {
		t.Fatal(err)
	}
	wait()
	clocks.set(start.Add(2 * time.Hour))
	if err := clock.control(focus.Pause, start.Add(2*time.Hour)); !errors.Is(err, focus.ErrChanged) {
		t.Errorf("paused a phase that is up: %v", err)
	}
	if f := ended(); f.Title != "Break is over" || f.Body != "Ready for focus session 3 of 4?" {
		t.Errorf("%+v", f)
	}
}
