package platform

import (
	"errors"
	"time"
)

// MaxCalendarItems is the most items of each kind ReadUpNext hands over,
// so a long list of overdue reminders stays bounded before it is sorted.
const MaxCalendarItems = 500

// CalendarEvent is an event from the OS calendars.
type CalendarEvent struct {
	Title string
	// Calendar is the name of its calendar.
	Calendar   string
	Start, End time.Time
	AllDay     bool
	// Declined: the organizer canceled it, or the user declined it.
	Declined bool
}

// CalendarReminder is an incomplete reminder with a due date.
type CalendarReminder struct {
	Title string
	// List is the name of its list.
	List string
	// Due is when it is due. With DayOnly, it is due on a day without a
	// time, and Due is that date at midnight UTC.
	Due     time.Time
	DayOnly bool
}

// Why one kind of calendar item could not be read. Any other error says
// why itself.
var (
	// ErrCalendarNotAsked: the user has not answered the OS yet.
	ErrCalendarNotAsked = errors.New("calendar access not asked for")
	// ErrCalendarDenied: the user said no, or allowed adding events only.
	ErrCalendarDenied = errors.New("calendar access denied")
	// ErrCalendarRestricted: a policy, such as Screen Time, blocks access.
	ErrCalendarRestricted = errors.New("calendar access restricted")
	// ErrOpenedByAnotherApp: TinyDash is not its own responsible process.
	// macOS would check the access and the usage strings of the app that
	// started it, so TinyDash neither reads nor asks.
	ErrOpenedByAnotherApp = errors.New("opened by another app")
	// ErrCannotAskForCalendars: the app lacks the usage strings, without
	// which macOS ends an app that asks.
	ErrCannotAskForCalendars = errors.New("no calendar usage strings")
)

// Agenda is what ReadUpNext read: each kind, or why it could not.
type Agenda struct {
	Events    []CalendarEvent
	EventsErr error
	Reminders []CalendarReminder
	// RemindersErr is why reminders could not be read, or nil.
	RemindersErr error
}
