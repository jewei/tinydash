package app

import (
	"errors"
	"reflect"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

func TestReadsUpNextOnlyWhileItIsOnAndTheOSHasCalendars(t *testing.T) {
	zone := time.FixedZone("UTC+8", 8*3600)
	now := time.Date(2026, 3, 10, 14, 0, 0, 0, zone)
	var windows [][2]time.Time
	agenda := platform.Agenda{
		Events:       []platform.CalendarEvent{{Title: "Review", Calendar: "Work", Start: now.Add(time.Hour), End: now.Add(2 * time.Hour)}},
		RemindersErr: platform.ErrCalendarDenied,
	}
	read := func(from, to time.Time) (platform.Agenda, bool) {
		windows = append(windows, [2]time.Time{from, to})
		return agenda, true
	}

	s := settings.Default()
	if got := loadUpNext(s, now, now.Location(), read); got != nil || windows != nil {
		t.Errorf("Up Next is off by default: %+v, read %v", got, windows)
	}
	s.ShowUpNext = true
	// The clock's time comes in UTC; today ends at midnight in zone.
	got := loadUpNext(s, now.UTC(), zone, read)
	midnight := time.Date(2026, 3, 11, 0, 0, 0, 0, zone)
	if len(windows) != 1 || !windows[0][0].Equal(now) || !windows[0][1].Equal(midnight) {
		t.Errorf("read %v", windows)
	}
	want := &launcher.UpNext{
		Items: []launcher.UpNextItem{{Kind: core.UpNextEvent, Title: "Review", Time: "15:00–16:00", Source: "Work",
			Action: core.Do("Open Calendar", core.OpenUpNext{Kind: core.UpNextEvent})}},
		Problems:  []string{"TinyDash cannot read your reminders. In the privacy settings of macOS, allow TinyDash under Reminders."},
		ChangesAt: now.Add(time.Hour),
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("Up Next %+v, want %+v", got, want)
	}

	// Windows and Linux read no calendars: no widget.
	none := func(time.Time, time.Time) (platform.Agenda, bool) { return platform.Agenda{}, false }
	if got := loadUpNext(s, now, now.Location(), none); got != nil {
		t.Errorf("without calendars: %+v", got)
	}
}

// Each reason the platform gives reaches the widget's words, and another
// failure says why itself.
func TestNamesEachReasonACalendarCannotBeRead(t *testing.T) {
	now := time.Date(2026, 3, 10, 14, 0, 0, 0, time.UTC)
	s := settings.Default()
	s.ShowUpNext = true
	for _, c := range []struct {
		err  error
		want string
	}{
		{platform.ErrCalendarNotAsked, "TinyDash needs your permission to read your calendars and reminders."},
		{platform.ErrCalendarDenied, "TinyDash cannot read your calendars and reminders. In the privacy settings of macOS, allow TinyDash under Calendars and under Reminders."},
		{platform.ErrCalendarRestricted, "Access to your calendars and reminders is restricted on this Mac, for example by Screen Time."},
		{platform.ErrOpenedByAnotherApp, "Another app opened TinyDash, so macOS would check that app's access to your calendars and reminders. Quit TinyDash, then open it from Finder or Launchpad."},
		{platform.ErrCannotAskForCalendars, "This build of TinyDash cannot ask to read your calendars and reminders. Use the installed app."},
		{errors.New("macOS could not load its calendar framework."), "Could not read your calendars and reminders. macOS could not load its calendar framework."},
	} {
		got := loadUpNext(s, now, now.Location(), func(time.Time, time.Time) (platform.Agenda, bool) {
			return platform.Agenda{EventsErr: c.err, RemindersErr: c.err}, true
		})
		if got == nil || !reflect.DeepEqual(got.Problems, []string{c.want}) {
			t.Errorf("%v: %+v", c.err, got)
		}
		if asks := got != nil && got.Ask != nil; asks != (c.err == platform.ErrCalendarNotAsked) {
			t.Errorf("%v: offers to ask %v", c.err, asks)
		}
	}
}
