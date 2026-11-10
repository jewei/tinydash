package platform

import (
	"errors"
	"math"
	"os"
	"os/exec"
	"runtime"
	"sync"
	"sync/atomic"
	"time"

	"github.com/ebitengine/purego"
	"github.com/ebitengine/purego/objc"
	"github.com/jewei/tinydash/internal/core"
)

// calendarUsageKeys are the usage strings macOS needs before it asks for
// calendars and reminders: the full-access ones from macOS 14, the others
// before. mygo.json puts them in the app's Info.plist.
var calendarUsageKeys = []string{
	"NSCalendarsFullAccessUsageDescription",
	"NSCalendarsUsageDescription",
	"NSRemindersFullAccessUsageDescription",
	"NSRemindersUsageDescription",
}

// remindersWait: reminders come back on a queue of EventKit's; stop
// waiting after this.
const remindersWait = 2 * time.Second

// EventKit's numbers.
const (
	ekEntityEvent    = 0
	ekEntityReminder = 1

	ekStatusNotDetermined = 0
	ekStatusRestricted    = 1
	// ekStatusFullAccess was Authorized before macOS 14. Denied (2) and
	// write only (4) cannot read.
	ekStatusFullAccess = 3

	ekEventCanceled       = 3
	ekParticipantDeclined = 3

	// nsDateComponentUndefined is NSIntegerMax: a component that is not set.
	nsDateComponentUndefined = math.MaxInt
)

var eventKit = sync.OnceValue(func() error {
	_, err := purego.Dlopen("/System/Library/Frameworks/EventKit.framework/EventKit", purego.RTLD_GLOBAL|purego.RTLD_NOW)
	return err
})

// askingForCalendars is set while a request for access waits for the user.
var askingForCalendars atomic.Bool

// calendarStore is the store that reads Up Next. Apple advises keeping
// one, since a store is slow to make and to release. A store made before
// the user allowed access keeps seeing nothing, so a new answer needs a new
// store: readable says which kinds the store could read when it was made.
// The mutex lets one read at a time use it.
var calendarStore struct {
	sync.Mutex
	store    objc.ID
	readable [2]bool
}

// ReadUpNext reads the events between from and to and the incomplete
// reminders due before to, each kind with why it cannot be read, such as
// access the user has not given. It never asks for access. It reads
// nothing when TinyDash is not its own responsible process: macOS would
// check the access of the terminal or other app that started it. ok is
// false where TinyDash reads no calendars. It waits up to two seconds for
// reminders; call it off the main thread.
func ReadUpNext(from, to time.Time) (Agenda, bool) {
	if eventKit() != nil {
		failed := errors.New("macOS could not load its calendar framework.")
		return Agenda{EventsErr: failed, RemindersErr: failed}, true
	}
	// An autorelease pool belongs to the thread that made it.
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
	defer pool.Send(objc.RegisterName("drain"))

	own := ownResponsibleProcess()
	a := Agenda{EventsErr: calendarAccess(ekEntityEvent, own), RemindersErr: calendarAccess(ekEntityReminder, own)}
	if a.EventsErr != nil && a.RemindersErr != nil {
		return a, true
	}
	// Held until the read ends, so one read at a time uses the store.
	calendarStore.Lock()
	defer calendarStore.Unlock()
	readable := [2]bool{a.EventsErr == nil, a.RemindersErr == nil}
	if calendarStore.store == 0 || calendarStore.readable != readable {
		if calendarStore.store != 0 {
			calendarStore.store.Send(objc.RegisterName("release"))
		}
		calendarStore.store = newEventStore()
		calendarStore.readable = readable
	}
	store, end := calendarStore.store, nsDate(to)
	if a.EventsErr == nil {
		a.Events = readEvents(store, nsDate(from), end)
	}
	if a.RemindersErr == nil {
		a.Reminders, a.RemindersErr = readReminders(store, end)
	}
	return a, true
}

// AskCalendarAccess asks the user to let TinyDash read calendars and
// reminders, for each kind not answered yet, and calls done on another
// thread once both are answered. It returns at once. It does nothing while
// an earlier request waits; when TinyDash is not its own responsible
// process, as macOS would ask for the other app, and end TinyDash if that
// app lacks the usage strings; and in a build without the usage strings,
// as macOS ends an app that asks without them.
func AskCalendarAccess(done func()) {
	if eventKit() != nil {
		return
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
	defer pool.Send(objc.RegisterName("drain"))

	own := ownResponsibleProcess()
	var kinds []int
	for _, kind := range []int{ekEntityEvent, ekEntityReminder} {
		if errors.Is(calendarAccess(kind, own), ErrCalendarNotAsked) {
			kinds = append(kinds, kind)
		}
	}
	if len(kinds) == 0 || askingForCalendars.Swap(true) {
		return
	}
	// The store must live until EventKit answers: the last answer
	// releases it.
	store := newEventStore()
	var waiting atomic.Int32
	waiting.Store(int32(len(kinds)))
	// EventKit calls this once per request, on a queue of its own, and
	// keeps its own copy of the block until then.
	answered := objc.NewBlock(func(objc.Block, bool, objc.ID) {
		if waiting.Add(-1) == 0 {
			store.Send(objc.RegisterName("release"))
			askingForCalendars.Store(false)
			done()
		}
	})
	defer answered.Release()
	modern := objc.Send[bool](store, objc.RegisterName("respondsToSelector:"), objc.RegisterName("requestFullAccessToEventsWithCompletion:"))
	for _, kind := range kinds {
		switch {
		case modern && kind == ekEntityReminder:
			store.Send(objc.RegisterName("requestFullAccessToRemindersWithCompletion:"), answered)
		case modern:
			store.Send(objc.RegisterName("requestFullAccessToEventsWithCompletion:"), answered)
		default:
			store.Send(objc.RegisterName("requestAccessToEntityType:completion:"), kind, answered)
		}
	}
}

// OpenCalendarApp opens the app that holds an Up Next item: Calendar for
// an event, Reminders for a reminder.
func OpenCalendarApp(kind core.UpNextKind) error {
	app := "com.apple.iCal"
	if kind == core.UpNextReminder {
		app = "com.apple.reminders"
	}
	return run(exec.Command("/usr/bin/open", "-b", app))
}

func newEventStore() objc.ID {
	return objc.ID(objc.GetClass("EKEventStore")).Send(objc.RegisterName("alloc")).Send(objc.RegisterName("init"))
}

// calendarAccess says whether TinyDash may read one kind, from the answer
// macOS keeps. It never asks. own comes from ownResponsibleProcess. Call
// it inside an autorelease pool.
func calendarAccess(kind int, own bool) error {
	if !own {
		// The saved answer would be the other app's, so it is not read.
		return ErrOpenedByAnotherApp
	}
	switch objc.Send[int](objc.ID(objc.GetClass("EKEventStore")), objc.RegisterName("authorizationStatusForEntityType:"), kind) {
	case ekStatusFullAccess:
		return nil
	case ekStatusNotDetermined:
		if hasCalendarUsageStrings(objc.ID(objc.GetClass("NSBundle")).Send(objc.RegisterName("mainBundle"))) {
			return ErrCalendarNotAsked
		}
		return ErrCannotAskForCalendars
	case ekStatusRestricted:
		return ErrCalendarRestricted
	}
	return ErrCalendarDenied
}

// ownResponsibleProcess reports whether TinyDash is its own responsible
// process. macOS checks the access and the usage strings of the
// responsible process, not of the process that reads or asks, and a
// program that a terminal or another app starts belongs to that app. An
// app that launchd starts is its own: from Finder, Launchpad, the Dock, or
// `open`. So is the login item. The answer cannot change while TinyDash
// runs, so it is looked up once.
var ownResponsibleProcess = sync.OnceValue(func() bool {
	return responsibleForItself(findResponsibleFor(), int32(os.Getpid()))
})

// findResponsibleFor finds responsibility_get_pid_responsible_for_pid, a
// private libSystem function: the process that macOS holds responsible for
// a process, or -1 when it cannot tell. It is nil where this macOS lacks
// it. It only reads what the kernel keeps about the process.
func findResponsibleFor() func(pid int32) int32 {
	symbol, err := purego.Dlsym(purego.RTLD_DEFAULT, "responsibility_get_pid_responsible_for_pid")
	if err != nil || symbol == 0 {
		return nil
	}
	var responsibleFor func(pid int32) int32
	purego.RegisterFunc(&responsibleFor, symbol)
	return responsibleFor
}

// responsibleForItself reports whether responsibleFor names pid as its own
// responsible process. Without the function, TinyDash cannot tell, so it
// takes the safe answer.
func responsibleForItself(responsibleFor func(int32) int32, pid int32) bool {
	return responsibleFor != nil && responsibleFor(pid) == pid
}

// hasCalendarUsageStrings reports whether bundle carries every usage
// string. macOS ends its own responsible process when it asks without
// them. The packaged app has them; a run from source does not.
func hasCalendarUsageStrings(bundle objc.ID) bool {
	for _, key := range calendarUsageKeys {
		if bundle.Send(objc.RegisterName("objectForInfoDictionaryKey:"), nsString(key)) == 0 {
			return false
		}
	}
	return true
}

func nsDate(t time.Time) objc.ID {
	return objc.ID(objc.GetClass("NSDate")).Send(objc.RegisterName("dateWithTimeIntervalSince1970:"), float64(t.UnixMilli())/1000)
}

// goTime reads an NSDate, rounded to the millisecond, as seconds in a
// double miss some milliseconds by a hair.
func goTime(date objc.ID) time.Time {
	seconds := objc.Send[float64](date, objc.RegisterName("timeIntervalSince1970"))
	return time.UnixMilli(int64(math.Round(seconds * 1000)))
}

func count(array objc.ID) int {
	return objc.Send[int](array, objc.RegisterName("count"))
}

func readEvents(store, from, to objc.ID) []CalendarEvent {
	// The predicate comes from this store, as the search requires. The
	// search may return nil, which counts no items.
	predicate := store.Send(objc.RegisterName("predicateForEventsWithStartDate:endDate:calendars:"), from, to, objc.ID(0))
	found := store.Send(objc.RegisterName("eventsMatchingPredicate:"), predicate)
	var events []CalendarEvent
	for i := range min(count(found), MaxCalendarItems) {
		if e, ok := readEvent(found.Send(objc.RegisterName("objectAtIndex:"), i)); ok {
			events = append(events, e)
		}
	}
	return events
}

// readEvent reads an event. Its title may be nil, and so may its dates,
// though the headers do not promise them.
func readEvent(event objc.ID) (CalendarEvent, bool) {
	start := event.Send(objc.RegisterName("startDate"))
	end := event.Send(objc.RegisterName("endDate"))
	if start == 0 || end == 0 {
		return CalendarEvent{}, false
	}
	return CalendarEvent{
		Title:    goString(event.Send(objc.RegisterName("title"))),
		Calendar: calendarName(event),
		Start:    goTime(start),
		End:      goTime(end),
		AllDay:   objc.Send[bool](event, objc.RegisterName("isAllDay")),
		Declined: canceledOrDeclined(event),
	}, true
}

// canceledOrDeclined reports whether the organizer canceled the event, or
// the user declined it. EventKit returns both, and Calendar hides them.
func canceledOrDeclined(event objc.ID) bool {
	if objc.Send[int](event, objc.RegisterName("status")) == ekEventCanceled {
		return true
	}
	attendees := event.Send(objc.RegisterName("attendees"))
	for i := range count(attendees) {
		attendee := attendees.Send(objc.RegisterName("objectAtIndex:"), i)
		if objc.Send[bool](attendee, objc.RegisterName("isCurrentUser")) &&
			objc.Send[int](attendee, objc.RegisterName("participantStatus")) == ekParticipantDeclined {
			return true
		}
	}
	return false
}

// readReminders reads incomplete reminders due before to, overdue ones
// included.
func readReminders(store, to objc.ID) ([]CalendarReminder, error) {
	found := make(chan []CalendarReminder, 1)
	// EventKit calls this once, on a queue of its own, with an array that
	// lives for the call.
	handler := objc.NewBlock(func(_ objc.Block, list objc.ID) {
		pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
		defer pool.Send(objc.RegisterName("drain"))
		var reminders []CalendarReminder
		for i := range min(count(list), MaxCalendarItems) {
			if r, ok := readReminder(list.Send(objc.RegisterName("objectAtIndex:"), i)); ok {
				reminders = append(reminders, r)
			}
		}
		// The reader may have stopped waiting.
		select {
		case found <- reminders:
		default:
		}
	})
	// EventKit keeps its own copy of the block.
	defer handler.Release()
	predicate := store.Send(objc.RegisterName("predicateForIncompleteRemindersWithDueDateStarting:ending:calendars:"), objc.ID(0), to, objc.ID(0))
	fetch := store.Send(objc.RegisterName("fetchRemindersMatchingPredicate:completion:"), predicate, handler)
	timer := time.NewTimer(remindersWait)
	defer timer.Stop()
	select {
	case reminders := <-found:
		return reminders, nil
	case <-timer.C:
		if fetch != 0 {
			store.Send(objc.RegisterName("cancelFetchRequest:"), fetch)
		}
		return nil, errors.New("Reminders did not answer in time. Open the launcher again.")
	}
}

// readReminder reads a reminder with a due date. Its title may be nil,
// though the header does not promise it.
func readReminder(reminder objc.ID) (CalendarReminder, bool) {
	components := reminder.Send(objc.RegisterName("dueDateComponents"))
	if components == 0 {
		return CalendarReminder{}, false
	}
	due, dayOnly, ok := dueDate(components)
	if !ok {
		return CalendarReminder{}, false
	}
	return CalendarReminder{
		Title:   goString(reminder.Send(objc.RegisterName("title"))),
		List:    calendarName(reminder),
		Due:     due,
		DayOnly: dayOnly,
	}, true
}

// dueDate reads when a reminder is due: a day without a time, at midnight
// UTC, or a moment. EventKit keeps due dates in the Gregorian calendar,
// whichever calendar the user chose in the region settings.
func dueDate(components objc.ID) (due time.Time, dayOnly, ok bool) {
	if objc.Send[int](components, objc.RegisterName("hour")) == nsDateComponentUndefined {
		year := objc.Send[int](components, objc.RegisterName("year"))
		month := objc.Send[int](components, objc.RegisterName("month"))
		day := objc.Send[int](components, objc.RegisterName("day"))
		due = time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
		// A component that is not set, or out of range, is no date.
		ok = due.Year() == year && int(due.Month()) == month && due.Day() == day
		return due, true, ok
	}
	identifier, err := constant("NSCalendarIdentifierGregorian")
	if err != nil {
		return time.Time{}, false, false
	}
	gregorian := objc.ID(objc.GetClass("NSCalendar")).Send(objc.RegisterName("calendarWithIdentifier:"), identifier)
	// The calendar uses the components' own time zone when they have one,
	// and the system's otherwise.
	date := gregorian.Send(objc.RegisterName("dateFromComponents:"), components)
	if date == 0 {
		return time.Time{}, false, false
	}
	return goTime(date), false, true
}

// calendarName is the name of the calendar or reminder list that holds
// item.
func calendarName(item objc.ID) string {
	return goString(item.Send(objc.RegisterName("calendar")).Send(objc.RegisterName("title")))
}
