package settings

import (
	"strconv"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/settings"
)

// The focus timer is off by default, as in the Rust app, and its switch
// and lengths save to their settings, within the Rust app's limits.
func TestTurnsTheFocusTimerOnAndSetsItsLengths(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, widgets)
	if checked(t, tt, "Focus timer") {
		t.Fatal("the focus timer is on by default")
	}
	flip(t, tt, "Focus timer")
	if !f.last(t).ShowFocusTimer {
		t.Fatal("the switch did not turn the timer on")
	}
	for _, c := range []struct {
		field, typed string
		saved        func(settings.Settings) int
		want         int
	}{
		{"Focus minutes", "50", func(s settings.Settings) int { return int(s.FocusMinutes) }, 50},
		{"Focus minutes", "500", func(s settings.Settings) int { return int(s.FocusMinutes) }, settings.FocusMinutesMax},
		{"Short break minutes", "0", func(s settings.Settings) int { return int(s.ShortBreakMinutes) }, 1},
		{"Long break minutes", "90", func(s settings.Settings) int { return int(s.LongBreakMinutes) }, settings.BreakMinutesMax},
		{"Sessions before a long break", "3", func(s settings.Settings) int { return int(s.SessionsBeforeLongBreak) }, 3},
		{"Sessions before a long break", "12", func(s settings.Settings) int { return int(s.SessionsBeforeLongBreak) }, settings.SessionsMax},
	} {
		typeInto(t, tt, c.field, c.typed, true)
		if got := c.saved(f.last(t)); got != c.want {
			t.Errorf("%s %s: saved %d, want %d", c.field, c.typed, got, c.want)
		}
		if info, _ := tt.AccessByRole(ui.RoleTextField, c.field); info.Value != strconv.Itoa(c.want) {
			t.Errorf("%s shows %q", c.field, info.Value)
		}
	}
}
