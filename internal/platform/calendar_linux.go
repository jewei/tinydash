package platform

import (
	"errors"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

// ReadUpNext reads no calendars here: Up Next reads only the macOS ones.
func ReadUpNext(from, to time.Time) (Agenda, bool) { return Agenda{}, false }

// AskCalendarAccess does nothing here.
func AskCalendarAccess(done func()) {}

// OpenCalendarApp fails here.
func OpenCalendarApp(kind core.UpNextKind) error {
	return errors.New("Up Next works only on macOS.")
}
