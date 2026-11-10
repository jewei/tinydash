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

// The hint for an empty clipboard shows only when no other widget does,
// as the Solid app had it: the focus timer counts.
func TestTheClipboardHintGivesWayToTheFocusTimer(t *testing.T) {
	f := newFake(always())
	m, tt := open(t, f)
	focus := running(tt.Now())
	f.widgets = Widgets{ClipCards: true, Focus: &focus}
	m.Show("")
	tt.Frame()
	if tt.HasText("Copy a color, a Unix time, or JSON to see it here.") {
		t.Errorf("the hint shows beside the timer: %q", tt.Texts())
	}
}

// A phase that ends while the launcher is away loads nothing: a load reads
// the clipboard for its card. The next show loads the widgets anyway.
func TestAPhaseEndLoadsTheWidgetsOnlyWhileShown(t *testing.T) {
	f := newFake(always())
	m, _ := showTimer(t, f, running)
	m.WidgetsChanged()
	if f.widgetLoads != 2 {
		t.Fatalf("loads %d while shown, want 2", f.widgetLoads)
	}
	m.Hidden()
	m.WidgetsChanged()
	if f.widgetLoads != 2 {
		t.Errorf("loaded the widgets while hidden: %d loads", f.widgetLoads)
	}
}

// With every widget on, the cards do not fit, so the pane scrolls and each
// card keeps its height; with fewer, the note fills the room left, from the
// pane's first frame.
func TestThePaneScrollsAndTheNoteFillsTheRoomLeft(t *testing.T) {
	f := newFake(always())
	m, tt := open(t, f)
	card := colorCard
	focus := running(tt.Now())
	f.widgets = Widgets{ClipCards: true, Card: &card, Clocks: true, Focus: &focus,
		Disk: &Disk{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 182e9}, Note: &Note{Text: "note"}}
	m.Show("")
	tt.Frame()
	pane, _ := tt.AccessByRole(ui.RoleRegion, "Widgets")
	timer, _ := tt.AccessByRole(ui.RoleGroup, "Focus timer")
	if timer.Bounds.H < 120 {
		t.Errorf("the timer card shrank to %v high in a full pane %+v", timer.Bounds.H, pane.Bounds)
	}
	color, _ := tt.AccessByRole(ui.RoleGroup, "Color on the clipboard")
	if hsl, ok := tt.Find("hsl(164, 41%, 31%)"); !ok || hsl.Y+hsl.H > color.Bounds.Y+color.Bounds.H {
		t.Errorf("the color card %+v cuts its HSL row %+v", color.Bounds, hsl)
	}

	f.widgets = Widgets{Clocks: true, Note: &Note{Text: "note"}}
	tt.Type("x") // the pane goes, and comes back new
	tt.Key(0, ui.KeyBackspace)
	note, _ := tt.AccessByRole(ui.RoleGroup, "Notepad")
	pane, _ = tt.AccessByRole(ui.RoleRegion, "Widgets")
	if bottom := note.Bounds.Y + note.Bounds.H; bottom < pane.Bounds.Y+pane.Bounds.H-24 {
		t.Errorf("the note ends at %v, short of the pane's end at %v", bottom, pane.Bounds.Y+pane.Bounds.H)
	}
}

// A bar above the pane that goes away makes the pane taller, and the note
// fills the new room at once, not at some later frame.
func TestTheNoteFollowsThePanesNewHeight(t *testing.T) {
	f := newFake(always(safari))
	f.warnings = []string{"The tray icon is unavailable."}
	f.widgets = Widgets{Clocks: true, Note: &Note{Text: "note"}}
	_, tt := showLauncher(t, f)
	fills := func(when string) {
		t.Helper()
		note, _ := tt.AccessByRole(ui.RoleGroup, "Notepad")
		pane, _ := tt.AccessByRole(ui.RoleRegion, "Widgets")
		if gap := pane.Bounds.Y + pane.Bounds.H - note.Bounds.Y - note.Bounds.H; gap < 0 || gap > 24 {
			t.Errorf("%s: the note ends %v short of the pane's end", when, gap)
		}
	}
	pressButton(t, tt, "Dismiss")
	fills("after the warning went")
	f.fail = func(core.Action) error { return errors.New("Safari is gone.") }
	tt.Key(0, ui.KeyEnter)
	fills("after a notice came")
}

// Mod+J into a note below the fold of a full pane scrolls it into view, so
// the user sees what they type.
func TestMovingToTheNoteShowsItInAFullPane(t *testing.T) {
	f := newFake(always())
	m, tt := open(t, f)
	card := colorCard
	focus := running(tt.Now())
	f.widgets = Widgets{ClipCards: true, Card: &card, Clocks: true, Focus: &focus,
		Disk: &Disk{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 182e9}, Note: &Note{Text: "note"}}
	m.Show("")
	tt.Frame()
	tt.Key(ui.Cmd, ui.KeyJ)
	pane, _ := tt.AccessByRole(ui.RoleRegion, "Widgets")
	note, _ := tt.AccessByRole(ui.RoleGroup, "Notepad")
	if !tt.Focused("Notepad") || note.Bounds.Y+note.Bounds.H > pane.Bounds.Y+pane.Bounds.H+1 {
		t.Errorf("the note %+v is below the pane %+v", note.Bounds, pane.Bounds)
	}
}

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

// A load of the widgets that started before a control holds the old
// state; it must not put it back.
func TestALoadFromBeforeAControlDoesNotUndoIt(t *testing.T) {
	f := newFake(always())
	f.holdWidgets = true
	m, tt := showTimer(t, f, running)
	old := running(tt.Now())
	f.loading[0](Widgets{Focus: &old})
	m.Stale()
	paused := FocusTimer{Phase: "Focus", Paused: true, Remaining: 20 * time.Minute, Total: 25 * time.Minute, Session: 1, Sessions: 4,
		Actions: []FocusAction{{"Resume Timer", fakeStart}}}
	f.widgets.Focus = &paused
	tt.Key(ui.Cmd, ui.KeyP)
	f.loading[1](Widgets{Focus: &old}) // read before the control
	tt.Frame()
	if !tt.HasText("Resume Timer") {
		t.Errorf("an older load put back the running timer: %q", tt.Texts())
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
	if !slices.Equal(f.controls, []FocusControl{fakePause, fakeStart}) {
		t.Errorf("controls %v", f.controls)
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
