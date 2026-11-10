package upnext

import (
	"errors"
	"reflect"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/zoneinfo"
)

var utc8 = time.FixedZone("UTC+8", 8*3600)

// now is 14:00 on 2026-03-10 in UTC+8.
var now = time.Date(2026, 3, 10, 14, 0, 0, 0, utc8)

// at is a time on the day of now.
func at(hour, minute int) time.Time { return time.Date(2026, 3, 10, hour, minute, 0, 0, utc8) }

var midnight = time.Date(2026, 3, 11, 0, 0, 0, 0, utc8)

func event(title string, start, end time.Time) Event {
	return Event{Title: title, Calendar: "Work", Start: start, End: end}
}

func allDay(title string) Event {
	e := event(title, at(0, 0), at(23, 59))
	e.AllDay = true
	return e
}

func dueAt(title string, t time.Time) Reminder { return Reminder{Title: title, List: "Home", Due: t} }

// dueOn is a reminder due on a day without a time, as the platform reads
// it: the date at midnight UTC.
func dueOn(title string, year int, month time.Month, day int) Reminder {
	return Reminder{Title: title, List: "Home", Due: time.Date(year, month, day, 0, 0, 0, 0, time.UTC), DayOnly: true}
}

func rows(v View) [][2]string {
	var out [][2]string
	for _, it := range v.Items {
		out = append(out, [2]string{it.Time, it.Title})
	}
	return out
}

func wantRows(t *testing.T, v View, want ...[2]string) {
	t.Helper()
	if got := rows(v); !reflect.DeepEqual(got, want) && (len(got) != 0 || len(want) != 0) {
		t.Errorf("rows %q, want %q", got, want)
	}
}

func TestReadsFromNowToMidnight(t *testing.T) {
	from, to := Window(now)
	if !from.Equal(now) || !to.Equal(midnight) {
		t.Errorf("window %v to %v", from, to)
	}
}

func TestListsAllDayEventsFirstThenTheRestOfToday(t *testing.T) {
	v := Build(Agenda{Events: []Event{
		event("Lunch", at(12, 0), at(13, 0)),
		event("Review", at(16, 30), at(17, 0)),
		event("Standup", at(13, 30), at(14, 30)),
		allDay("Holiday"),
		event("Flight", at(15, 0), at(15, 0)),
	}}, now)
	wantRows(t, v, [2]string{"All day", "Holiday"}, [2]string{"Now", "Standup"}, [2]string{"15:00", "Flight"}, [2]string{"16:30–17:00", "Review"})
	first := v.Items[0]
	if first.Kind != core.UpNextEvent || first.Source != "Work" || first.Action != core.Do("Open Calendar", core.OpenUpNext{Kind: core.UpNextEvent}) {
		t.Errorf("first %+v", first)
	}
	if v.Problems != nil || v.Ask != nil {
		t.Errorf("problems %q, ask %+v", v.Problems, v.Ask)
	}
}

func TestShowsAtMostSixEventsAndSixReminders(t *testing.T) {
	var a Agenda
	for n := range 9 {
		a.Events = append(a.Events, event("Event "+string(rune('0'+n)), at(15, n), at(16, 0)))
		a.Reminders = append(a.Reminders, dueAt("Task "+string(rune('0'+n)), at(18, n)))
	}
	v := Build(a, now)
	count := map[core.UpNextKind]int{}
	for _, it := range v.Items {
		count[it.Kind]++
	}
	if count[core.UpNextEvent] != maxEvents || count[core.UpNextReminder] != maxReminders {
		t.Errorf("counts %v", count)
	}
	if v.Items[5].Title != "Event 5" || v.Items[11].Title != "Task 5" {
		t.Errorf("rows %q", rows(v))
	}
}

func TestLeavesOutCanceledAndDeclinedEvents(t *testing.T) {
	// Enough of them to fill the list, were they kept.
	var events []Event
	for range maxEvents {
		e := event("Skipped", at(14, 30), at(15, 0))
		e.Declined = true
		events = append(events, e)
	}
	events = append(events, event("Review", at(16, 0), at(17, 0)))
	v := Build(Agenda{Events: events}, now)
	wantRows(t, v, [2]string{"16:00–17:00", "Review"})
	if !v.ChangesAt.Equal(at(16, 0)) {
		t.Errorf("changes at %v", v.ChangesAt)
	}
}

func TestOrdersRemindersByDueDateAndMarksOverdueOnes(t *testing.T) {
	v := Build(Agenda{Reminders: []Reminder{
		dueAt("Call back", at(16, 0)),
		dueOn("Water plants", 2026, 3, 10),
		dueOn("Pay rent", 2026, 3, 9),
		dueAt("Send report", at(9, 15)),
		dueOn("Next week", 2026, 3, 11),
		dueAt("Late tonight", midnight),
	}}, now)
	wantRows(t, v, [2]string{"Overdue", "Pay rent"}, [2]string{"Overdue", "Send report"}, [2]string{"Today", "Water plants"}, [2]string{"16:00", "Call back"})
	if v.Items[0].Source != "Home" || v.Items[0].Action != core.Do("Open Reminders", core.OpenUpNext{Kind: core.UpNextReminder}) {
		t.Errorf("first %+v", v.Items[0])
	}
}

// A reminder due on a day keeps its date west of UTC too: its date is a
// UTC midnight, which is the evening before there.
func TestReadsDueDaysInTheirOwnDateWestOfUTC(t *testing.T) {
	hawaii := time.FixedZone("UTC-10", -10*3600)
	evening := time.Date(2026, 3, 10, 20, 0, 0, 0, hawaii)
	v := Build(Agenda{Reminders: []Reminder{
		dueOn("Water plants", 2026, 3, 10),
		dueOn("Pay rent", 2026, 3, 9),
		dueOn("Next week", 2026, 3, 11),
	}}, evening)
	wantRows(t, v, [2]string{"Overdue", "Pay rent"}, [2]string{"Today", "Water plants"})
}

func TestLoadsAgainWhenTheNextItemStartsEndsOrFallsDue(t *testing.T) {
	reminders := []Reminder{dueAt("Call back", at(14, 40))}
	soon := Build(Agenda{Events: []Event{event("Standup", at(13, 30), at(14, 30)), allDay("Off")}, Reminders: reminders}, now)
	if !soon.ChangesAt.Equal(at(14, 30)) {
		t.Errorf("soon: %v", soon.ChangesAt)
	}
	later := Build(Agenda{Events: []Event{event("Review", at(15, 0), at(16, 0))}, Reminders: reminders}, now)
	if !later.ChangesAt.Equal(at(14, 40)) {
		t.Errorf("later: %v", later.ChangesAt)
	}
	free := Build(Agenda{Events: []Event{allDay("Off")}}, now)
	if !free.ChangesAt.Equal(midnight) {
		t.Errorf("free: %v", free.ChangesAt)
	}
	next := Build(Agenda{Events: []Event{event("Review", at(15, 0), at(16, 0))}}, now.Add(61*time.Minute))
	wantRows(t, next, [2]string{"Now", "Review"})
	if !next.ChangesAt.Equal(at(16, 0)) {
		t.Errorf("next: %v", next.ChangesAt)
	}
}

func TestLoadsAgainWhenAReminderLeftOutFallsDue(t *testing.T) {
	// At 16:00 the last one becomes overdue and moves to the top.
	var reminders []Reminder
	for range maxReminders {
		reminders = append(reminders, dueOn("Task", 2026, 3, 10))
	}
	reminders = append(reminders, dueAt("Call back", at(16, 0)))
	before := Build(Agenda{Reminders: reminders}, now)
	for _, it := range before.Items {
		if it.Title == "Call back" {
			t.Error("the limit kept Call back")
		}
	}
	if !before.ChangesAt.Equal(at(16, 0)) {
		t.Errorf("changes at %v", before.ChangesAt)
	}
	after := Build(Agenda{Reminders: reminders}, now.Add(2*time.Hour))
	if after.Items[0].Title != "Call back" || after.Items[0].Time != "Overdue" {
		t.Errorf("rows %q", rows(after))
	}
}

func TestCutsLongTitlesAndNamesEmptyOnes(t *testing.T) {
	long := strings.Repeat("word ", 40)
	v := Build(Agenda{Events: []Event{
		event(long, at(15, 0), at(16, 0)),
		event(" \n", at(17, 0), at(18, 0)),
	}}, now)
	title := v.Items[0].Title
	if utf8.RuneCountInString(title) != maxTitleRunes || !strings.HasSuffix(title, "…") || strings.Contains(title, "  ") {
		t.Errorf("title %q", title)
	}
	if v.Items[1].Title != "Untitled" {
		t.Errorf("empty title %q", v.Items[1].Title)
	}
}

func TestSaysWhyAKindCannotShowAndOffersToAsk(t *testing.T) {
	allowAccess := core.Do("Allow Access", core.AskCalendarAccess{})
	for _, c := range []struct {
		name     string
		agenda   Agenda
		problems []string
		ask      bool
		rows     [][2]string
	}{
		{"both denied", Agenda{EventsErr: ErrDenied, RemindersErr: ErrDenied},
			[]string{"TinyDash cannot read your calendars and reminders. In the privacy settings of macOS, allow TinyDash under Calendars and under Reminders."}, false, nil},
		{"reminders denied", Agenda{Events: []Event{event("Review", at(15, 0), at(16, 0))}, RemindersErr: ErrDenied},
			[]string{"TinyDash cannot read your reminders. In the privacy settings of macOS, allow TinyDash under Reminders."}, false, [][2]string{{"15:00–16:00", "Review"}}},
		{"waiting", Agenda{EventsErr: ErrNotAsked, RemindersErr: ErrRestricted},
			[]string{"TinyDash needs your permission to read your calendars.", "Access to your reminders is restricted on this Mac, for example by Screen Time."}, true, nil},
		// Without the usage strings, macOS would end TinyDash if it asked.
		{"cannot ask", Agenda{EventsErr: ErrCannotAsk, RemindersErr: ErrCannotAsk},
			[]string{"This build of TinyDash cannot ask to read your calendars and reminders. Use the installed app."}, false, nil},
		// macOS would check the access of the app that opened TinyDash, so
		// there is nothing to allow, only a way to open it as itself.
		{"opened by another app", Agenda{EventsErr: ErrOpenedByAnotherApp, RemindersErr: ErrOpenedByAnotherApp},
			[]string{"Another app opened TinyDash, so macOS would check that app's access to your calendars and reminders. Quit TinyDash, then open it from Finder or Launchpad."}, false, nil},
		{"reminders failed", Agenda{RemindersErr: errors.New("Reminders did not answer in time. Open the launcher again.")},
			[]string{"Could not read your reminders. Reminders did not answer in time. Open the launcher again."}, false, nil},
		{"both failed alike", Agenda{EventsErr: errors.New("EventKit is missing."), RemindersErr: errors.New("EventKit is missing.")},
			[]string{"Could not read your calendars and reminders. EventKit is missing."}, false, nil},
		{"one asked, one failed", Agenda{EventsErr: ErrNotAsked, RemindersErr: errors.New("access not asked for")},
			[]string{"TinyDash needs your permission to read your calendars.", "Could not read your reminders. access not asked for"}, true, nil},
	} {
		v := Build(c.agenda, now)
		if !reflect.DeepEqual(v.Problems, c.problems) {
			t.Errorf("%s: problems %q", c.name, v.Problems)
		}
		if (v.Ask != nil) != c.ask || v.Ask != nil && *v.Ask != allowAccess {
			t.Errorf("%s: ask %+v", c.name, v.Ask)
		}
		wantRows(t, v, c.rows...)
	}
}

// Chile moves its clocks from 00:00 to 01:00 on 2026-09-06, so that day
// starts at 01:00.
func TestTheDayEndsAtTheNextMidnightEvenWhenAClockChangeSkipsIt(t *testing.T) {
	santiago, err := zoneinfo.Load("America/Santiago")
	if err != nil {
		t.Fatal(err)
	}
	evening := time.Date(2026, 9, 5, 22, 0, 0, 0, santiago)
	from, to := Window(evening)
	if to.Sub(from) != 2*time.Hour {
		t.Errorf("window %v", to.Sub(from))
	}
	if got := to.UTC().Format(time.RFC3339); got != "2026-09-06T04:00:00Z" {
		t.Errorf("end of day %s", got)
	}
}
