package app

import "github.com/jewei/tinydash/internal/platform"

// askCalendarAccess asks the OS to let Up Next read calendars and
// reminders, and loads the widgets again once the user answers. It
// returns at once.
func (a *App) askCalendarAccess() {
	platform.AskCalendarAccess(func() { a.panel.Update(a.model.Stale) })
}
