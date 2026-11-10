package launcher

import (
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

var (
	openCalendar  = core.Do("Open Calendar", core.OpenUpNext{Kind: core.UpNextEvent})
	openReminders = core.Do("Open Reminders", core.OpenUpNext{Kind: core.UpNextReminder})
	allowAccess   = core.Do("Allow Access", core.AskCalendarAccess{})
)

// agenda is a day with two events and an overdue reminder, which changes
// in an hour.
func agenda(now time.Time) *UpNext {
	return &UpNext{
		Items: []UpNextItem{
			{Kind: core.UpNextEvent, Title: "Standup", Time: "Now", Source: "Work", Action: openCalendar},
			{Kind: core.UpNextEvent, Title: "Review", Time: "16:30–17:00", Source: "Work", Action: openCalendar},
			{Kind: core.UpNextReminder, Title: "Pay rent", Time: "Overdue", Source: "Home", Action: openReminders},
		},
		ChangesAt: now.Add(time.Hour),
	}
}

func TestShowsUpNextAndOpensTheAppOfAnItem(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{UpNext: agenda(time.Now())}
	_, tt := showLauncher(t, f)
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Up Next"); !ok || !hasPane(tt) {
		t.Fatalf("no Up Next; texts %q", tt.Texts())
	}
	for _, text := range []string{"Now", "Standup", "16:30–17:00", "Review", "Work", "Overdue", "Pay rent", "Home"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	if tt.HasText("Nothing else today") {
		t.Error("says nothing else today beside items")
	}
	pressButton(t, tt, "Overdue Pay rent Home")
	wantRuns(t, f, ran{core.OpenUpNext{Kind: core.UpNextReminder}, ""})
	if !tt.Focused("Search") {
		t.Error("the click took the focus from the search field")
	}
}

func TestSaysWhyUpNextCannotShowAndAsksForAccess(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{UpNext: &UpNext{
		Problems:  []string{"TinyDash needs your permission to read your calendars and reminders."},
		Ask:       &allowAccess,
		ChangesAt: time.Now().Add(time.Hour),
	}}
	_, tt := showLauncher(t, f)
	if !tt.HasText("TinyDash needs your permission to read your calendars and reminders.") || tt.HasText("Nothing else today") {
		t.Errorf("texts %q", tt.Texts())
	}
	pressButton(t, tt, "Allow Access")
	wantRuns(t, f, ran{core.AskCalendarAccess{}, ""})
}

func TestSaysWhenNothingElseIsOnToday(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{UpNext: &UpNext{ChangesAt: time.Now().Add(time.Hour)}}
	_, tt := showLauncher(t, f)
	if !tt.HasText("Nothing else today") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// The actions menu lists each Up Next action once, while the pane shows.
func TestListsEachUpNextActionInTheActionsMenu(t *testing.T) {
	f := newFake(always())
	u := agenda(time.Now())
	u.Ask = &allowAccess
	f.widgets = Widgets{UpNext: u}
	m, tt := showLauncher(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	var labels []string
	for _, item := range m.menu.items {
		if item.label == "Open Calendar" || item.label == "Open Reminders" || item.label == "Allow Access" {
			labels = append(labels, item.label)
		}
	}
	if len(labels) != 3 || labels[0] != "Open Calendar" || labels[1] != "Open Reminders" || labels[2] != "Allow Access" {
		t.Errorf("menu %q", labels)
	}
	chooseItem(t, tt, "Open Calendar")
	wantRuns(t, f, ran{core.OpenUpNext{Kind: core.UpNextEvent}, ""})

	// A typed query hides the pane, and its actions with it.
	tt.Type("x")
	tt.Key(ui.Cmd, ui.KeyK)
	for _, item := range m.menu.items {
		if item.label == "Open Calendar" {
			t.Error("the menu offers Up Next without the pane")
		}
	}
}

// Up Next changes as an event starts or ends: the pane loads again then,
// but only while the launcher shows, as each show loads anyway.
func TestLoadsUpNextAgainWhenItChangesWhileTheLauncherShows(t *testing.T) {
	f := newFake(always())
	m, tt := open(t, f)
	f.widgets = Widgets{UpNext: agenda(tt.Now())}
	m.Show("")
	tt.Frame()
	tt.Advance(59 * time.Minute)
	if f.widgetLoads != 1 {
		t.Fatalf("loaded %d times before the change", f.widgetLoads)
	}
	f.widgets = Widgets{UpNext: agenda(tt.Now().Add(time.Hour))}
	tt.Advance(time.Minute)
	if f.widgetLoads != 2 {
		t.Fatalf("loaded %d times at the change", f.widgetLoads)
	}
	m.Hidden()
	tt.Advance(3 * time.Hour)
	// A change of the data while the launcher hides, such as a copy,
	// reads no calendars either.
	m.Stale()
	if f.widgetLoads != 2 {
		t.Errorf("loaded %d times while hidden", f.widgetLoads)
	}
	m.Show("")
	if f.widgetLoads != 3 {
		t.Errorf("loaded %d times after the launcher showed again", f.widgetLoads)
	}
	// Shown again as it was, after an action that hid it failed, it
	// loads again as time passes.
	m.Hidden()
	m.Shown()
	f.widgets = Widgets{UpNext: agenda(tt.Now())}
	tt.Advance(time.Hour)
	if f.widgetLoads != 4 {
		t.Errorf("loaded %d times after the launcher showed as it was", f.widgetLoads)
	}
}

// A slow load is not started again on each frame once Up Next changed.
func TestWaitsForASlowLoadBeforeLoadingUpNextAgain(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{UpNext: agenda(time.Now())}
	m, tt := showLauncher(t, f)
	f.holdWidgets = true
	tt.Advance(time.Hour)
	tt.Advance(time.Minute)
	if f.widgetLoads != 2 || len(f.loading) != 1 {
		t.Fatalf("%d loads, %d waiting", f.widgetLoads, len(f.loading))
	}
	f.loading[0](Widgets{UpNext: agenda(tt.Now())})
	tt.Frame()
	if f.widgetLoads != 2 || m.widgetsLoading {
		t.Errorf("%d loads after the slow one ended", f.widgetLoads)
	}
}

// Before the first load, a change of the data loads the widgets even while
// the launcher hides, so its first show has them.
func TestLoadsTheWidgetsBeforeTheFirstShow(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Clocks: true}
	m, _ := open(t, f)
	m.Stale()
	m.Stale()
	if f.widgetLoads != 1 {
		t.Errorf("%d loads while hidden", f.widgetLoads)
	}
}
