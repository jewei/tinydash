package calc

import (
	"testing"

	"github.com/jewei/tinydash/internal/feature/datetime"
	"github.com/jewei/tinydash/internal/golden"
)

// isDateOrClock is a copy of datetime.IsDateOrClock, because features may
// not import each other. This keeps the two from drifting apart.
func TestDateCheckMatchesTheDatetimeFeature(t *testing.T) {
	for _, feature := range []string{"datetime", "calculator"} {
		for _, row := range golden.Rows[struct {
			Query string `json:"query"`
		}](t, feature) {
			if got, want := isDateOrClock(row.Query), datetime.IsDateOrClock(row.Query); got != want {
				t.Errorf("%q: calc says %v, datetime says %v", row.Query, got, want)
			}
		}
	}
}
