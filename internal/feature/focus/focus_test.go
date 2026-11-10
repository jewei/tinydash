package focus

import (
	"errors"
	"slices"
	"testing"
	"time"
)

var lengths = Lengths{Focus: 25 * time.Minute, ShortBreak: 5 * time.Minute, LongBreak: 15 * time.Minute, Sessions: 2}

// zero is the start of the tests' clock.
var zero = time.Date(2026, 11, 10, 9, 0, 0, 0, time.UTC)

func at(minutes int) time.Time { return zero.Add(time.Duration(minutes) * time.Minute) }

func labels(t *Timer) []string {
	var out []string
	for _, a := range t.Actions() {
		out = append(out, a.Label)
	}
	return out
}

func start(tb testing.TB, t *Timer, now time.Time) {
	tb.Helper()
	if err := t.Control(Start, lengths, now); err != nil {
		tb.Fatal(err)
	}
}

func TestRunsACycleOfSessionsAndBreaks(t *testing.T) {
	timer := New(lengths)
	if got := labels(&timer); !slices.Equal(got, []string{"Start Focus"}) {
		t.Errorf("%q", got)
	}
	start(t, &timer, at(0))
	if !timer.EndsAt.Equal(at(25)) {
		t.Errorf("ends at %v", timer.EndsAt)
	}
	if _, ok := timer.FinishIfDue(lengths, at(25).Add(-time.Millisecond)); ok {
		t.Error("finished early")
	}

	done, ok := timer.FinishIfDue(lengths, at(25))
	if !ok || done != (Finished{"Focus session done", "Time for a 5-minute break."}) {
		t.Errorf("%+v %v", done, ok)
	}
	if timer.State != Done || !slices.Equal(labels(&timer), []string{"Start Break", "Skip Break", "Reset Timer"}) {
		t.Errorf("%v %q", timer.State, labels(&timer))
	}

	start(t, &timer, at(30))
	if timer.Phase != ShortBreak || timer.Total != 5*time.Minute || timer.Phase.String() != "Short break" {
		t.Errorf("%+v", timer)
	}
	if got := labels(&timer); !slices.Equal(got, []string{"Pause Timer", "Skip Break", "Reset Timer"}) {
		t.Errorf("%q", got)
	}
	over, ok := timer.FinishIfDue(lengths, at(35))
	if !ok || over != (Finished{"Break is over", "Ready for focus session 2 of 2?"}) {
		t.Errorf("%+v", over)
	}
	if timer.Phase != Focus || timer.State != Idle || timer.Session != 2 {
		t.Errorf("%+v", timer)
	}

	// The last session of the cycle earns the long break, then session 1.
	start(t, &timer, at(0))
	done, _ = timer.FinishIfDue(lengths, at(25))
	if done.Body != "Time for a 15-minute long break." {
		t.Errorf("%q", done.Body)
	}
	start(t, &timer, at(25))
	if timer.Phase != LongBreak || timer.Phase.String() != "Long break" {
		t.Errorf("%v", timer.Phase)
	}
	if _, ok := timer.FinishIfDue(lengths, at(40)); !ok || timer.Session != 1 {
		t.Errorf("%+v", timer)
	}
}

func TestPausesResumesSkipsAndResets(t *testing.T) {
	timer := New(lengths)
	start(t, &timer, at(0))
	if err := timer.Control(Pause, lengths, at(10)); err != nil {
		t.Fatal(err)
	}
	if timer.Remaining != 15*time.Minute || timer.Left(at(50)) != 15*time.Minute {
		t.Errorf("%v", timer.Remaining)
	}
	if got := labels(&timer); !slices.Equal(got, []string{"Resume Timer", "Reset Timer"}) {
		t.Errorf("%q", got)
	}
	// A paused timer never finishes, however long it waits.
	if _, ok := timer.FinishIfDue(lengths, at(99)); ok {
		t.Error("a paused timer finished")
	}
	start(t, &timer, at(20))
	if !timer.EndsAt.Equal(at(35)) {
		t.Errorf("%v", timer.EndsAt)
	}

	if err := timer.Control(SkipBreak, lengths, at(0)); !errors.Is(err, ErrChanged) {
		t.Errorf("skipped a break during focus: %v", err)
	}
	timer.FinishIfDue(lengths, at(35))
	if err := timer.Control(SkipBreak, lengths, at(0)); err != nil {
		t.Fatal(err)
	}
	if timer.Phase != Focus || timer.Session != 2 {
		t.Errorf("%+v", timer)
	}

	if err := timer.Control(Reset, lengths, at(0)); err != nil || timer != New(lengths) {
		t.Errorf("%+v %v", timer, err)
	}
	if err := timer.Control(Pause, lengths, at(0)); !errors.Is(err, ErrChanged) {
		t.Errorf("paused an idle timer: %v", err)
	}
}

// Every control in every state either changes the timer as the state
// allows or leaves it as it was.
func TestARefusedControlChangesNothing(t *testing.T) {
	timers := map[string]Timer{"idle": New(lengths)}
	running := New(lengths)
	start(t, &running, at(0))
	timers["running"] = running
	paused := running
	_ = paused.Control(Pause, lengths, at(5))
	timers["paused"] = paused
	done := running
	done.FinishIfDue(lengths, at(25))
	timers["done"] = done
	allowed := map[string][]Control{
		"idle":    {Start, Reset},
		"running": {Pause, Reset},
		"paused":  {Start, Reset},
		"done":    {Start, SkipBreak, Reset},
	}
	for name, timer := range timers {
		for _, c := range []Control{Start, Pause, SkipBreak, Reset} {
			got := timer
			err := got.Control(c, lengths, at(6))
			if want := slices.Contains(allowed[name], c); (err == nil) != want {
				t.Errorf("%s, control %d: %v", name, c, err)
			}
			if err != nil && got != timer {
				t.Errorf("%s, control %d: changed to %+v", name, c, got)
			}
		}
	}
}

func TestNewLengthsWaitForTheNextPhase(t *testing.T) {
	longer := lengths
	longer.Focus, longer.Sessions = 50*time.Minute, 1
	timer := New(lengths)
	timer.SetLengths(longer)
	if timer.Remaining != 50*time.Minute || timer.Sessions != 1 {
		t.Errorf("%+v", timer)
	}

	running := New(lengths)
	start(t, &running, at(0))
	running.SetLengths(longer)
	if !running.EndsAt.Equal(at(25)) || running.Total != 25*time.Minute {
		t.Errorf("%+v", running)
	}
}

// Fewer sessions in a cycle move a later session back to the new last.
func TestFewerSessionsKeepTheSessionInTheCycle(t *testing.T) {
	timer := New(lengths)
	start(t, &timer, at(0))
	timer.FinishIfDue(lengths, at(25))
	_ = timer.Control(SkipBreak, lengths, at(25))
	if timer.Session != 2 {
		t.Fatalf("%+v", timer)
	}
	one := lengths
	one.Sessions = 1
	timer.SetLengths(one)
	if timer.Session != 1 || timer.Sessions != 1 {
		t.Errorf("%+v", timer)
	}
}

// A phase whose time is up, but which FinishIfDue has not ended yet,
// refuses a pause: it would pause at 00:00 and never notify. Reset still
// works.
func TestAPhaseThatIsUpWaitsToFinish(t *testing.T) {
	timer := New(lengths)
	start(t, &timer, at(0))
	before := timer
	if err := timer.Control(Pause, lengths, at(26)); !errors.Is(err, ErrChanged) || timer != before {
		t.Errorf("%+v %v", timer, err)
	}
	if _, ok := timer.FinishIfDue(lengths, at(26)); !ok {
		t.Error("the phase did not finish")
	}

	start(t, &timer, at(30))
	if err := timer.Control(Reset, lengths, at(40)); err != nil || timer != New(lengths) {
		t.Errorf("%+v %v", timer, err)
	}
}
