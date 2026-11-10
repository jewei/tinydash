package launcher

import (
	"errors"
	"slices"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// The fake's codes for the controls; the launcher only hands them back.
const (
	fakeStart FocusControl = iota + 1
	fakePause
	fakeSkip
	fakeReset
)

// showTimer shows the launcher with the focus timer that timer makes from
// the tester's time.
func showTimer(t *testing.T, f *fake, timer func(now time.Time) FocusTimer) (*Model, *ui.Tester) {
	t.Helper()
	m, tt := open(t, f)
	focus := timer(tt.Now())
	f.widgets = Widgets{Focus: &focus}
	m.Show("")
	tt.Frame()
	return m, tt
}

func running(now time.Time) FocusTimer {
	return FocusTimer{
		Phase: "Focus", EndsAt: now.Add(25 * time.Minute), Total: 25 * time.Minute, Session: 1, Sessions: 4,
		Actions: []FocusAction{{"Pause Timer", fakePause}, {"Reset Timer", fakeReset}},
	}
}

func TestTheFocusTimerCountsDownEachSecond(t *testing.T) {
	f := newFake(always())
	_, tt := showTimer(t, f, running)
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Focus timer"); !ok || !hasPane(tt) {
		t.Fatalf("no focus timer; texts %q", tt.Texts())
	}
	for _, text := range []string{"Focus", "25:00", "Session 1 of 4", "Pause Timer", "Reset Timer"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	if timer, ok := tt.AccessByRole(ui.RoleTimer, "25:00"); !ok {
		t.Errorf("no timer role: %+v", timer)
	}
	tt.Advance(time.Second)
	if !tt.HasText("24:59") {
		t.Errorf("after a second: %q", tt.Texts())
	}
	// The last second reads 00:01 until it is over.
	tt.Advance(24*time.Minute + 58*time.Second + 500*time.Millisecond)
	if !tt.HasText("00:01") {
		t.Errorf("half a second before the end: %q", tt.Texts())
	}
	tt.Advance(500 * time.Millisecond)
	if !tt.HasText("00:00") {
		t.Errorf("at the end: %q", tt.Texts())
	}
	if f.widgetLoads != 1 {
		t.Errorf("loaded the widgets %d times; the view counts down alone", f.widgetLoads)
	}
}

func TestTheFocusTimerNamesEachState(t *testing.T) {
	for _, c := range []struct {
		name  string
		timer FocusTimer
		want  []string
	}{
		{"paused", FocusTimer{Phase: "Focus", Paused: true, Remaining: 15 * time.Minute, Total: 25 * time.Minute, Session: 2, Sessions: 4},
			[]string{"Focus · Paused", "15:00", "Session 2 of 4"}},
		{"done", FocusTimer{Phase: "Focus", Done: true, Total: 25 * time.Minute, Session: 4, Sessions: 4},
			[]string{"Session done", "Done", "Session 4 of 4"}},
		{"break", FocusTimer{Phase: "Long break", Break: true, Remaining: 15 * time.Minute, Total: 15 * time.Minute, Session: 4, Sessions: 4},
			[]string{"Long break", "15:00"}},
		// A wall clock set back a day shows no more than the phase.
		{"clock set back", FocusTimer{Phase: "Focus", EndsAt: time.Now().Add(24 * time.Hour), Total: 25 * time.Minute, Session: 1, Sessions: 4},
			[]string{"25:00"}},
	} {
		t.Run(c.name, func(t *testing.T) {
			f := newFake(always())
			_, tt := showTimer(t, f, func(time.Time) FocusTimer { return c.timer })
			for _, text := range c.want {
				if !tt.HasText(text) {
					t.Errorf("no %q in %q", text, tt.Texts())
				}
			}
		})
	}
}

// Mod+P runs the main action while the pane shows the timer, and the pane
// shows the timer's new state at once, so a second press acts on it, also
// while another action runs. Typed text hides the pane, and Mod+P with it.
func TestModPRunsTheTimersMainAction(t *testing.T) {
	f := newFake(always(safari))
	f.hold = true
	_, tt := showTimer(t, f, running)
	paused := FocusTimer{Phase: "Focus", Paused: true, Remaining: 20 * time.Minute, Total: 25 * time.Minute, Session: 1, Sessions: 4,
		Actions: []FocusAction{{"Resume Timer", fakeStart}, {"Reset Timer", fakeReset}}}
	f.controlFocus = func(FocusControl) error {
		f.widgets.Focus = &paused
		return nil
	}
	tt.Key(0, ui.KeyEnter) // an action that has not ended yet
	tt.Key(ui.Cmd, ui.KeyP)
	if !tt.HasText("Resume Timer") || !tt.HasText("Focus · Paused") {
		t.Errorf("texts %q", tt.Texts())
	}
	tt.Key(ui.Cmd, ui.KeyP)
	if !slices.Equal(f.controls, []FocusControl{fakePause, fakeStart}) || f.widgetLoads != 1 {
		t.Errorf("controls %v, loads %d", f.controls, f.widgetLoads)
	}
	tt.Type("x")
	tt.Key(ui.Cmd, ui.KeyP)
	if len(f.controls) != 2 {
		t.Errorf("Mod+P ran a hidden timer: %v", f.controls)
	}
}

// A click on a control runs it and leaves the keyboard in the search field.
func TestAClickRunsAControlAndKeepsTyping(t *testing.T) {
	f := newFake(always())
	_, tt := showTimer(t, f, running)
	pressButton(t, tt, "Reset Timer")
	if !slices.Equal(f.controls, []FocusControl{fakeReset}) || !tt.Focused("Search") {
		t.Errorf("controls %v, search focused %v", f.controls, tt.Focused("Search"))
	}
}

func TestAControlThatFailsSaysWhy(t *testing.T) {
	f := newFake(always())
	f.controlFocus = func(FocusControl) error { return errors.New("The focus timer changed. Try again.") }
	_, tt := showTimer(t, f, running)
	tt.Key(ui.Cmd, ui.KeyP)
	if !tt.HasText("The focus timer changed. Try again.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// The actions menu lists the timer's actions after the selected result's,
// with Mod+P on the main one.
func TestTheMenuListsTheTimersActions(t *testing.T) {
	f := newFake(always(safari))
	m, tt := showTimer(t, f, running)
	tt.Key(ui.Cmd, ui.KeyK)
	var labels []string
	for _, item := range m.menu.items {
		labels = append(labels, item.label)
	}
	pause := slices.Index(labels, "Pause Timer")
	if pause <= slices.Index(labels, safari.Actions[0].Label) || labels[pause+1] != "Reset Timer" ||
		!slices.Equal(m.menu.items[pause].keys, []string{modKey, "P"}) || m.menu.items[pause+1].keys != nil {
		t.Fatalf("menu %q", labels)
	}
	chooseItem(t, tt, "Reset Timer")
	if !slices.Equal(f.controls, []FocusControl{fakeReset}) {
		t.Errorf("controls %v", f.controls)
	}

	// On another tab the pane is gone, and so are its actions.
	tt.Key(0, ui.KeyTab)
	tt.Key(ui.Cmd, ui.KeyK)
	for _, item := range m.menu.items {
		if item.label == "Pause Timer" {
			t.Errorf("the menu lists a hidden timer's action on %s", m.category)
		}
	}
	if m.category == core.CategoryAll {
		t.Error("Tab did not leave All")
	}
}

// Every text of the timer stays inside its card, also in a narrow pane and
// with the longest labels.
func TestTheFocusTimerTextStaysInItsCard(t *testing.T) {
	for _, width := range []int{760, 560} {
		f := newFake(always())
		m := New(f)
		tt := ui.NewTester(m.View, width, 480)
		focus := FocusTimer{Phase: "Short break", Break: true, Paused: true, Remaining: 119*time.Minute + 59*time.Second,
			Total: 120 * time.Minute, Session: 8, Sessions: 8,
			Actions: []FocusAction{{"Resume Timer", fakeStart}, {"Skip Break", fakeSkip}, {"Reset Timer", fakeReset}}}
		f.widgets = Widgets{Focus: &focus}
		m.Show("")
		tt.Frame()
		card, ok := tt.AccessByRole(ui.RoleGroup, "Focus timer")
		if !ok {
			t.Fatalf("no card; texts %q", tt.Texts())
		}
		for _, text := range []string{"Short break · Paused", "119:59", "Resume Timer", "Skip Break", "Reset Timer", "Session 8 of 8", "P"} {
			r, ok := tt.Find(text)
			if !ok {
				t.Errorf("width %d: no %q", width, text)
				continue
			}
			if r.X < card.Bounds.X || r.Y < card.Bounds.Y || r.X+r.W > card.Bounds.X+card.Bounds.W || r.Y+r.H > card.Bounds.Y+card.Bounds.H {
				t.Errorf("width %d: %q at %+v is outside the card at %+v", width, text, r, card.Bounds)
			}
		}
	}
}

// The time fits inside the ring in the app's display font, for every
// length that Settings allows.
func TestTheTimeFitsInsideTheRing(t *testing.T) {
	if err := kit.RegisterFonts(); err != nil {
		t.Fatal(err)
	}
	f := newFake(always())
	m := New(f)
	tt := ui.NewTester(m.View, 880, 560)
	for minutes := range 121 {
		focus := FocusTimer{Phase: "Focus", Remaining: time.Duration(minutes)*time.Minute + 58*time.Second, Total: 121 * time.Minute, Session: 1, Sessions: 4}
		f.widgets = Widgets{Focus: &focus}
		m.Show("")
		tt.Frame()
		shown := minutesAndSeconds(focus.Remaining)
		r, ok := tt.Find(shown)
		if !ok {
			t.Fatalf("no %q", shown)
		}
		// The ring is a circle of radius 28, and its stroke of 5 takes
		// 2.5 of it; the digits, about 12 high, need room at their
		// corners too.
		if r.W > ringRoom {
			t.Errorf("%q is %v wide, more than the ring holds", shown, r.W)
		}
	}
}

// ringRoom is the widest text that fits inside the ring with a margin.
const ringRoom = 47

func TestMinutesAndSecondsRoundUp(t *testing.T) {
	for d, want := range map[time.Duration]string{
		0:                                   "00:00",
		time.Millisecond:                    "00:01",
		time.Second:                         "00:01",
		time.Second + time.Millisecond:      "00:02",
		25 * time.Minute:                    "25:00",
		120 * time.Minute:                   "120:00",
		24*time.Minute + 59*time.Second:     "24:59",
		24*time.Minute + 58*time.Second + 1: "24:59",
	} {
		if got := minutesAndSeconds(d); got != want {
			t.Errorf("minutesAndSeconds(%v) = %q, want %q", d, got, want)
		}
	}
}
