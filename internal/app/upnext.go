package app

import (
	"errors"
	"time"

	"github.com/jewei/tinydash/internal/feature/upnext"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// loadUpNext reads the rest of today from the calendars while Up Next is
// on. It is nil while the widget is off, and where TinyDash reads no
// calendars. Today ends at midnight in zone, the computer's time zone now.
func loadUpNext(s settings.Settings, now time.Time, zone *time.Location, read func(from, to time.Time) (platform.Agenda, bool)) *launcher.UpNext {
	if !s.ShowUpNext {
		return nil
	}
	now = now.In(zone)
	agenda, ok := read(upnext.Window(now))
	if !ok {
		return nil
	}
	v := upnext.Build(agendaOf(agenda), now)
	u := &launcher.UpNext{Problems: v.Problems, Ask: v.Ask, ChangesAt: v.ChangesAt}
	for _, it := range v.Items {
		u.Items = append(u.Items, launcher.UpNextItem(it))
	}
	return u
}

// agendaOf hands what the platform read to the widget's logic.
func agendaOf(a platform.Agenda) upnext.Agenda {
	out := upnext.Agenda{EventsErr: calendarError(a.EventsErr), RemindersErr: calendarError(a.RemindersErr)}
	for _, e := range a.Events {
		out.Events = append(out.Events, upnext.Event(e))
	}
	for _, r := range a.Reminders {
		out.Reminders = append(out.Reminders, upnext.Reminder(r))
	}
	return out
}

// calendarErrors are the platform's reasons for a kind it could not read,
// as the widget names them.
var calendarErrors = []struct{ platform, widget error }{
	{platform.ErrCalendarNotAsked, upnext.ErrNotAsked},
	{platform.ErrCalendarDenied, upnext.ErrDenied},
	{platform.ErrCalendarRestricted, upnext.ErrRestricted},
	{platform.ErrOpenedByAnotherApp, upnext.ErrOpenedByAnotherApp},
	{platform.ErrCannotAskForCalendars, upnext.ErrCannotAsk},
}

// calendarError is the widget's name for err; another failure says why
// itself.
func calendarError(err error) error {
	for _, e := range calendarErrors {
		if errors.Is(err, e.platform) {
			return e.widget
		}
	}
	return err
}

// askCalendarAccess asks the OS to let Up Next read calendars and
// reminders, and loads the widgets again once the user answers. It
// returns at once.
func (a *App) askCalendarAccess() {
	platform.AskCalendarAccess(func() { a.panel.Update(a.model.Stale) })
}
