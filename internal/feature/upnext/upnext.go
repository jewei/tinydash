// Package upnext is the Up Next widget: the rest of today's events and the
// reminders due by the end of today, from the OS calendars. The platform
// reads them (EventKit on macOS); this package picks, orders, and labels
// them. The caller passes the time now, in the user's time zone, so tests
// need no clock. A port of src-tauri/src/features/up_next.rs.
package upnext

import (
	"errors"
	"fmt"
	"slices"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
)

const (
	// maxEvents is the most events the widget shows.
	maxEvents = 6
	// maxReminders is the most reminders the widget shows.
	maxReminders   = 6
	maxTitleRunes  = 80
	maxSourceRunes = 40
)

// Event is an event as the platform reads it.
type Event struct {
	Title string
	// Calendar is the name of its calendar.
	Calendar   string
	Start, End time.Time
	AllDay     bool
	// Declined: the organizer canceled it, or the user declined it, so it
	// is not on the user's day.
	Declined bool
}

// Reminder is an incomplete reminder with a due date, as the platform
// reads it.
type Reminder struct {
	Title string
	// List is the name of its list.
	List string
	// Due is when it is due. With DayOnly, it is due on a day without a
	// time, and only the date of Due counts, in Due's own location.
	Due     time.Time
	DayOnly bool
}

// Why one kind of item could not be read. Any other error is a failure
// that says why itself.
var (
	// ErrNotAsked: the user has not answered the OS yet.
	ErrNotAsked = errors.New("access not asked for")
	// ErrDenied: the user said no, or allowed adding events only.
	ErrDenied = errors.New("access denied")
	// ErrRestricted: a policy, such as Screen Time, blocks access.
	ErrRestricted = errors.New("access restricted")
	// ErrOpenedByAnotherApp: another app, such as a terminal, started
	// TinyDash. macOS would check that app's access and usage strings, so
	// TinyDash neither reads nor asks.
	ErrOpenedByAnotherApp = errors.New("opened by another app")
	// ErrCannotAsk: this build lacks the usage strings, without which
	// macOS ends an app that asks.
	ErrCannotAsk = errors.New("cannot ask")
)

// Agenda is what the platform read: each kind, or why it could not.
type Agenda struct {
	Events    []Event
	EventsErr error
	Reminders []Reminder
	// RemindersErr is why reminders could not be read, or nil.
	RemindersErr error
}

// Item is one row of the widget.
type Item struct {
	Kind  core.UpNextKind
	Title string
	// Time is when, in words: "All day", "Now", "10:30–11:00", "Today",
	// "16:00", or "Overdue".
	Time string
	// Source is the calendar or the reminder list.
	Source string
	// Action opens Calendar for an event, Reminders for a reminder.
	Action core.ResultAction
}

// View is what the Up Next widget shows.
type View struct {
	// Items are the events, then the reminders.
	Items []Item
	// Problems say why a kind of item cannot show, and how to fix it.
	Problems []string
	// Ask asks the OS for access, while the user has not answered.
	Ask *core.ResultAction
	// ChangesAt is when the list changes as time passes: an event starts
	// or ends, a reminder falls due, or the day ends. The pane loads
	// again then.
	ChangesAt time.Time
}

// Window is the time to read: from now to the end of today.
func Window(now time.Time) (from, to time.Time) {
	return now, endOfToday(now)
}

// Build picks, orders, and labels what the platform read.
func Build(a Agenda, now time.Time) View {
	zone := now.Location()
	end := endOfToday(now)
	today := date(now)
	changes := []time.Time{end}
	var v View

	events := slices.DeleteFunc(slices.Clone(a.Events), func(e Event) bool {
		return e.Declined || !e.End.After(now) || !e.Start.Before(end)
	})
	slices.SortStableFunc(events, func(x, y Event) int {
		switch {
		case x.AllDay != y.AllDay:
			if x.AllDay {
				return -1
			}
			return 1
		case !x.Start.Equal(y.Start):
			return x.Start.Compare(y.Start)
		case !x.End.Equal(y.End):
			return x.End.Compare(y.End)
		}
		return strings.Compare(x.Title, y.Title)
	})
	for _, e := range events[:min(len(events), maxEvents)] {
		label := "All day"
		if !e.AllDay {
			changes = append(changes, e.End)
			if e.Start.After(now) {
				changes = append(changes, e.Start)
				label = span(zone, e.Start, e.End)
			} else {
				label = "Now"
			}
		}
		v.Items = append(v.Items, item(core.UpNextEvent, e.Title, label, e.Calendar))
	}

	reminders := slices.DeleteFunc(slices.Clone(a.Reminders), func(r Reminder) bool {
		if r.DayOnly {
			return date(r.Due).After(today)
		}
		return !r.Due.Before(end)
	})
	overdue := func(r Reminder) bool {
		if r.DayOnly {
			return date(r.Due).Before(today)
		}
		return !r.Due.After(now)
	}
	// A reminder due on a day sorts at the start of that day here.
	due := func(r Reminder) time.Time {
		if r.DayOnly {
			return startOfDay(zone, r.Due.Year(), r.Due.Month(), r.Due.Day())
		}
		return r.Due
	}
	// Overdue ones first, so a reminder due today without a time, which
	// sorts at midnight, does not split them.
	slices.SortStableFunc(reminders, func(x, y Reminder) int {
		if ox, oy := overdue(x), overdue(y); ox != oy {
			if ox {
				return -1
			}
			return 1
		}
		if c := due(x).Compare(due(y)); c != 0 {
			return c
		}
		return strings.Compare(x.Title, y.Title)
	})
	// A reminder that falls due moves up among the overdue ones, so one
	// that the limit leaves out can come in then.
	var next time.Time
	for _, r := range reminders {
		if !r.DayOnly && r.Due.After(now) && (next.IsZero() || r.Due.Before(next)) {
			next = r.Due
		}
	}
	if !next.IsZero() {
		changes = append(changes, next)
	}
	for _, r := range reminders[:min(len(reminders), maxReminders)] {
		var label string
		switch {
		case overdue(r):
			label = "Overdue"
		case r.DayOnly:
			label = "Today"
		default:
			label = clock(zone, r.Due)
		}
		v.Items = append(v.Items, item(core.UpNextReminder, r.Title, label, r.List))
	}

	v.Problems = problems(a.EventsErr, a.RemindersErr)
	if errors.Is(a.EventsErr, ErrNotAsked) || errors.Is(a.RemindersErr, ErrNotAsked) {
		ask := core.Do("Allow Access", core.AskCalendarAccess{})
		v.Ask = &ask
	}
	v.ChangesAt = slices.MinFunc(changes, time.Time.Compare)
	return v
}

// openAction is the action that opens the app of an item.
func openAction(kind core.UpNextKind) core.ResultAction {
	label := "Open Calendar"
	if kind == core.UpNextReminder {
		label = "Open Reminders"
	}
	return core.Do(label, core.OpenUpNext{Kind: kind})
}

func item(kind core.UpNextKind, title, label, source string) Item {
	title = cut(title, maxTitleRunes)
	if title == "" {
		title = "Untitled"
	}
	return Item{Kind: kind, Title: title, Time: label, Source: cut(source, maxSourceRunes), Action: openAction(kind)}
}

// cut puts text on one line, with single spaces, and cuts it to max
// runes.
func cut(text string, max int) string {
	text = strings.Join(strings.Fields(text), " ")
	if utf8.RuneCountInString(text) <= max {
		return text
	}
	return string([]rune(text)[:max-1]) + "…"
}

// subject is what a problem is about, in words, and where the privacy
// settings list TinyDash for it. macOS 12 calls those settings System
// Preferences > Security & Privacy, and later versions System Settings >
// Privacy & Security, so the text names neither path.
type subject struct{ words, place string }

var (
	calendars = subject{"calendars", "under Calendars"}
	reminders = subject{"reminders", "under Reminders"}
	both      = subject{"calendars and reminders", "under Calendars and under Reminders"}
)

// problems is one line per problem; the same problem for both kinds is one
// line.
func problems(events, reminderErr error) []string {
	if events != nil && reminderErr != nil && sameProblem(events, reminderErr) {
		return []string{message(events, both)}
	}
	var out []string
	if events != nil {
		out = append(out, message(events, calendars))
	}
	if reminderErr != nil {
		out = append(out, message(reminderErr, reminders))
	}
	return out
}

func sameProblem(a, b error) bool {
	for _, known := range []error{ErrNotAsked, ErrDenied, ErrRestricted, ErrOpenedByAnotherApp, ErrCannotAsk} {
		if errors.Is(a, known) || errors.Is(b, known) {
			return errors.Is(a, known) && errors.Is(b, known)
		}
	}
	return a.Error() == b.Error()
}

func message(err error, s subject) string {
	switch {
	case errors.Is(err, ErrNotAsked):
		return fmt.Sprintf("TinyDash needs your permission to read your %s.", s.words)
	case errors.Is(err, ErrDenied):
		return fmt.Sprintf("TinyDash cannot read your %s. In the privacy settings of macOS, allow TinyDash %s.", s.words, s.place)
	case errors.Is(err, ErrRestricted):
		return fmt.Sprintf("Access to your %s is restricted on this Mac, for example by Screen Time.", s.words)
	case errors.Is(err, ErrOpenedByAnotherApp):
		return fmt.Sprintf("Another app opened TinyDash, so macOS would check that app's access to your %s. Quit TinyDash, then open it from Finder or Launchpad.", s.words)
	case errors.Is(err, ErrCannotAsk):
		return fmt.Sprintf("This build of TinyDash cannot ask to read your %s. Use the installed app.", s.words)
	}
	return fmt.Sprintf("Could not read your %s. %s", s.words, err.Error())
}

// span is "10:30–11:00", or one time for an event with no length.
func span(zone *time.Location, start, end time.Time) string {
	if !end.After(start) {
		return clock(zone, start)
	}
	return clock(zone, start) + "–" + clock(zone, end)
}

func clock(zone *time.Location, t time.Time) string { return t.In(zone).Format("15:04") }

// date is the calendar date of t in its own location, as a UTC midnight,
// so dates compare with Before and After.
func date(t time.Time) time.Time {
	return time.Date(t.Year(), t.Month(), t.Day(), 0, 0, 0, 0, time.UTC)
}

// endOfToday is the first moment of tomorrow in now's location.
func endOfToday(now time.Time) time.Time {
	tomorrow := date(now).AddDate(0, 0, 1)
	return startOfDay(now.Location(), tomorrow.Year(), tomorrow.Month(), tomorrow.Day())
}

// startOfDay is the first moment of a day in zone. Where a clock change
// skips midnight, the day starts at the first hour that exists.
func startOfDay(zone *time.Location, year int, month time.Month, day int) time.Time {
	for hour := range 3 {
		t := time.Date(year, month, day, hour, 0, 0, 0, zone)
		if t.Hour() == hour && t.Day() == day {
			return t
		}
	}
	return time.Date(year, month, day, 0, 0, 0, 0, time.UTC)
}
