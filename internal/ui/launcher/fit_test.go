package launcher

import (
	"strings"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// Every card of the pane keeps its texts and buttons inside it at the
// launcher's size, with the longest values its widget allows: a city
// setting of up to 100 characters, a long disk name, the longest phase of
// the focus timer, long event titles. Each card shows alone, so it is not
// scrolled out of view. A text that runs past the card's side, or a value
// that breaks onto a second line, fails.
func TestEveryCardKeepsItsTextInside(t *testing.T) {
	long := strings.Repeat("Llanfairpwllgwyngyll", 5) // 100 characters, one word
	rain := uint8(100)
	paused := FocusTimer{Phase: "Long break", Break: true, Paused: true, Remaining: 119*time.Minute + 59*time.Second,
		Total: 120 * time.Minute, Session: 8, Sessions: 8,
		Actions: []FocusAction{{"Resume Timer", fakeStart}, {"Skip Break", fakePause}, {"Reset Timer", fakeReset}}}
	color, unix, object := colorCard, timeCard, jsonCard()
	for _, c := range []struct {
		name, group string
		widgets     Widgets
		// oneLine are texts that must not wrap.
		oneLine []string
	}{
		{"clocks", "Clocks", Widgets{Clocks: true, Cities: []City{
			{Name: "Port Moresby", Difference: "+18h"}, {Name: "Ciudad Juárez", Difference: "−11h"}, {Name: "Chatham Islands", Difference: "+23h 45m"},
		}}, []string{"+23h 45m"}},
		{"up next", "Up Next", Widgets{UpNext: &UpNext{Items: []UpNextItem{
			{Kind: core.UpNextEvent, Title: strings.Repeat("Quarterly planning review ", 4), Time: "16:30–17:00", Source: "Work calendar with a long name", Action: openCalendar},
			{Kind: core.UpNextReminder, Title: long, Time: "Overdue", Source: "Home", Action: openReminders},
		}, Problems: []string{"Calendar access is off. Allow it under Calendars in System Settings > Privacy & Security."}}}, []string{"16:30–17:00"}},
		{"weather", "Weather", Widgets{Weather: &Weather{State: WeatherReady, City: long, Place: long + ", " + long,
			Temperature: -40, High: -38, Low: -45, Condition: "Thunderstorm with heavy hail", Icon: "thunder", RainChance: &rain, Offline: true}}, nil},
		{"weather not found", "Weather", Widgets{Weather: &Weather{State: WeatherNotFound, City: long}}, nil},
		{"weather loading", "Weather", Widgets{Weather: &Weather{State: WeatherLoading, City: long}}, nil},
		{"weather failed", "Weather", Widgets{Weather: &Weather{State: WeatherFailed, City: long, Message: "Check your internet connection, then refresh from the actions menu."}}, nil},
		{"focus timer", "Focus timer", Widgets{Focus: &paused}, []string{"Long break · Paused"}},
		{"disk space", "Disk space", Widgets{Disk: &Disk{Name: "Macintosh HD - Data", TotalBytes: 994e12, FreeBytes: 151e9}}, []string{"151 GB", "Low space"}},
		{"color card", "Color on the clipboard", Widgets{ClipCards: true, Card: &color}, []string{"hsl(164, 41%, 31%)"}},
		{"time card", "Unix time on the clipboard", Widgets{ClipCards: true, Card: &unix}, []string{"Wed 7 Oct 2026, 06:25:01 UTC"}},
		{"json card", "JSON on the clipboard", Widgets{ClipCards: true, Card: &object}, nil},
	} {
		t.Run(c.name, func(t *testing.T) {
			f := newFake(always())
			f.widgets = c.widgets
			m, tt := open(t, f)
			tt.SetSize(880, 560)
			m.Show("")
			tt.Frame()
			card, ok := tt.AccessByRole(ui.RoleGroup, c.group)
			if !ok {
				t.Fatalf("no %q card; texts %q", c.group, tt.Texts())
			}
			right := card.Bounds.X + card.Bounds.W
			for _, text := range tt.Texts() {
				r, _ := tt.Find(text)
				inside := r.X >= card.Bounds.X && r.X < right && r.Y >= card.Bounds.Y && r.Y < card.Bounds.Y+card.Bounds.H
				if inside && r.X+r.W > right+0.5 {
					t.Errorf("%q ends at x %v, past the card's side at %v", text, r.X+r.W, right)
				}
			}
			for _, text := range c.oneLine {
				if r, ok := tt.Find(text); !ok || r.H > 24 {
					t.Errorf("%q is %v high, more than one line; texts %q", text, r.H, tt.Texts())
				}
			}
			if b, ok := tt.AccessByRole(ui.RoleButton, "Open Settings"); ok && b.Bounds.X+b.Bounds.W > right+0.5 {
				t.Errorf("Open Settings %+v leaves the card %+v", b.Bounds, card.Bounds)
			}
		})
	}
}
