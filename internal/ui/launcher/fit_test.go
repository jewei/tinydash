package launcher

import (
	"errors"
	"image/color"
	"strings"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// Every card of the pane keeps its texts and buttons inside it at the
// launcher's size (880 × 560), in each of its states, with the longest
// values its widget allows: a city setting of 100 characters, a disk name
// as long, the longest phase of the focus timer, long event titles. Each
// card shows alone, so it is not scrolled out of view.
//
// The sizes are the elements' own, as Access gives them, not the part a
// card's clip leaves: a clipped text has its full width there, and a text
// pushed out entirely keeps its place. A text, a button named in buttons,
// or a button with the name of a text fails when it runs past the card's
// side; a value in oneLine fails when it wraps. A text that draws past its
// own box shows in the strip of pane to the card's right, which must stay
// the pane's color. The JSON card's key chips are cut at the card's side
// on purpose, as in 0.9.0.
func TestEveryCardKeepsItsTextInside(t *testing.T) {
	long := strings.Repeat("Llanfairpwllgwyngyll", 5) // 100 characters, one word
	rain := uint8(100)
	breakPaused := FocusTimer{Phase: "Short break", Break: true, Paused: true, Remaining: 59*time.Minute + 59*time.Second,
		Total: 60 * time.Minute, Session: 8, Sessions: 8,
		Actions: []FocusAction{{"Resume Timer", fakeStart}, {"Skip Break", fakePause}, {"Reset Timer", fakeReset}}}
	done := FocusTimer{Phase: "Focus", Done: true, Total: 120 * time.Minute, Session: 8, Sessions: 8,
		Actions: []FocusAction{{"Start Long Break", fakeStart}, {"Skip Break", fakePause}, {"Reset Timer", fakeReset}}}
	running := FocusTimer{Phase: "Focus", EndsAt: time.Now().Add(119 * time.Minute), Total: 120 * time.Minute, Session: 1, Sessions: 8,
		Actions: []FocusAction{{"Pause Timer", fakePause}, {"Reset Timer", fakeReset}}}
	allow := core.Do("Allow Access", core.AskCalendarAccess{})
	colorFound, object := colorCard, longJSONCard()
	millis := ClipCard{ID: "t2", Time: &ClipTime{Raw: "1791354301123", Milliseconds: true, Relative: "in 11 months, 3 weeks"},
		Rows: []CardRow{{"Local", "Wednesday 7 October 2026, 14:25:01.123"}, {"UTC", "Wednesday 7 October 2026, 06:25:01.123 UTC"}, {"ISO", "2026-10-07T06:25:01.123Z"}},
		Actions: []core.ResultAction{
			core.Do("Copy Local Time", core.Copy{Text: "a"}), core.Do("Copy UTC Time", core.Copy{Text: "b"}), core.Do("Copy ISO 8601", core.Copy{Text: "c"}),
		}}
	weather := func(w Weather) Widgets { return Widgets{Weather: &w} }
	for _, c := range []struct {
		name, group string
		role        ui.Role
		widgets     Widgets
		// json opens the full JSON view, which takes the pane's place.
		json bool
		// buttons are buttons that show no text of their name, such as
		// icon buttons; oneLine are texts that must not wrap.
		buttons, oneLine []string
	}{
		{name: "clocks", group: "Clocks", widgets: Widgets{Clocks: true, Cities: []City{
			{Name: "America/Argentina/ComodRivadavia", Difference: "−25h 45m"}, {Name: long, Difference: "+23h 45m"}, {Name: "Ciudad Juárez", Difference: "Same time"},
		}}, oneLine: []string{"−25h 45m", "+23h 45m", "Same time"}},
		{name: "clocks alone", group: "Clocks", widgets: Widgets{Clocks: true}},
		{name: "up next", group: "Up Next", widgets: Widgets{UpNext: &UpNext{Items: []UpNextItem{
			{Kind: core.UpNextEvent, Title: strings.Repeat("Quarterly planning review ", 4), Time: "16:30–17:00", Source: long, Action: openCalendar},
			{Kind: core.UpNextReminder, Title: long, Time: "Overdue", Source: "Home", Action: openReminders},
		}, Problems: []string{"Calendar access is off. Allow it under Calendars in System Settings > Privacy & Security."}, Ask: &allow}},
			oneLine: []string{"16:30–17:00", "Overdue"}},
		{name: "up next empty", group: "Up Next", widgets: Widgets{UpNext: &UpNext{}}},
		{name: "weather", group: "Weather", widgets: weather(Weather{State: WeatherReady, City: long, Place: long + ", " + long,
			Temperature: -40, High: -38, Low: -45, Condition: "Thunderstorm with heavy hail", Icon: "thunder", RainChance: &rain, Offline: true})},
		{name: "weather no city", group: "Weather", widgets: weather(Weather{State: WeatherNoCity}), buttons: []string{"Open Settings"}},
		{name: "weather not found", group: "Weather", widgets: weather(Weather{State: WeatherNotFound, City: long}), buttons: []string{"Open Settings"}},
		{name: "weather loading", group: "Weather", widgets: weather(Weather{State: WeatherLoading, City: long})},
		{name: "weather failed", group: "Weather", widgets: weather(Weather{State: WeatherFailed, City: long, Message: "Check your internet connection, then refresh from the actions menu."})},
		{name: "focus break", group: "Focus timer", widgets: Widgets{Focus: &breakPaused}, oneLine: []string{"Short break · Paused", "Session 8 of 8"}},
		{name: "focus done", group: "Focus timer", widgets: Widgets{Focus: &done}, oneLine: []string{"Session done"}},
		{name: "focus running", group: "Focus timer", widgets: Widgets{Focus: &running}},
		{name: "disk space", group: "Disk space", widgets: Widgets{Disk: &Disk{Name: long, TotalBytes: 994e12, FreeBytes: 151e9}},
			oneLine: []string{"151 GB", "Low space", "free of 994 TB"}},
		{name: "disk error", group: "Disk space", widgets: Widgets{Disk: &Disk{Err: errors.New("The folder " + long + " is gone.")}}},
		{name: "notepad", group: "Notepad", widgets: Widgets{Note: &Note{Text: long + " " + long}}},
		{name: "notepad error", group: "Notepad", widgets: Widgets{Note: &Note{Err: errors.New("The note table " + long + " is gone.")}}},
		{name: "color card", group: "Color on the clipboard", widgets: Widgets{ClipCards: true, Card: &colorFound},
			buttons: []string{"Dismiss", "Copy HEX", "Copy RGB", "Copy HSL"}, oneLine: []string{"hsl(164, 41%, 31%)"}},
		{name: "time card", group: "Unix time on the clipboard", widgets: Widgets{ClipCards: true, Card: &millis},
			buttons: []string{"Dismiss", "Copy Local", "Copy UTC", "Copy ISO"}},
		{name: "json card", group: "JSON on the clipboard", widgets: Widgets{ClipCards: true, Card: &object}, buttons: []string{"Dismiss"}},
		{name: "json view", group: "Clipboard JSON", role: ui.RoleDialog, widgets: Widgets{ClipCards: true, Card: &object}, json: true,
			buttons: []string{"Back to widgets"}},
		{name: "cards hint", group: "Widgets", role: ui.RoleRegion, widgets: Widgets{ClipCards: true}},
	} {
		t.Run(c.name, func(t *testing.T) {
			f := newFake(always())
			f.widgets = c.widgets
			m, tt := open(t, f)
			tt.SetSize(880, 560)
			m.Show("")
			tt.Frame()
			if c.json {
				tt.Key(ui.Cmd|ui.Shift, ui.KeyEnter)
			}
			role := c.role
			if role == 0 {
				role = ui.RoleGroup
			}
			card, ok := tt.AccessByRole(role, c.group)
			if !ok {
				t.Fatalf("no %q; texts %q", c.group, tt.Texts())
			}
			right, bottom := card.Bounds.X+card.Bounds.W, card.Bounds.Y+card.Bounds.H
			keys, _ := tt.AccessByRole(ui.RoleList, "Keys")
			check := func(what string, r ui.Rect) {
				t.Helper()
				if keys.Bounds.H > 0 && r.Y >= keys.Bounds.Y && r.Y < keys.Bounds.Y+keys.Bounds.H {
					return // a key chip, cut on purpose
				}
				in := r.X >= card.Bounds.X-0.5 && r.Y >= card.Bounds.Y-0.5 && r.Y < bottom
				if in && max(r.X, r.X+r.W) > right+0.5 {
					t.Errorf("%s %+v runs past the side of %q at x %v", what, r, c.group, right)
				}
			}
			// box is a text's own box. A text inside a named row, as an Up
			// Next item, has only its drawn one. A heading with its card's
			// name ("Up Next", "Clipboard JSON") gives the card's box, so
			// it goes unmeasured: both are short constant words.
			box := func(text string) (ui.Rect, bool) {
				if a, ok := tt.Access(text); ok {
					return a.Bounds, true
				}
				return tt.Find(text)
			}
			for _, text := range tt.Texts() {
				if r, ok := box(text); ok {
					check("text "+quote(text), r)
				}
				if b, ok := tt.AccessByRole(ui.RoleButton, text); ok {
					check("button "+quote(text), b.Bounds)
				}
			}
			for _, name := range c.buttons {
				b, ok := tt.AccessByRole(ui.RoleButton, name)
				if !ok {
					t.Errorf("no button %q", name)
					continue
				}
				check("button "+quote(name), b.Bounds)
			}
			// Nothing draws on the pane between the card and the window's
			// side.
			if img := tt.Image(); right+8 < 880 {
				scale := float32(img.Bounds().Dx()) / 880
				pane := img.RGBAAt(int((right+4)*scale), int((card.Bounds.Y-4)*scale))
				for y := card.Bounds.Y; y < bottom; y++ {
					for _, x := range []float32{right + 4, 876} {
						if got := img.RGBAAt(int(x*scale), int(y*scale)); !near(got, pane) {
							t.Fatalf("something draws past the side of %q at (%v, %v): %v, not the pane's %v", c.group, x, y, got, pane)
						}
					}
				}
			}
			for _, text := range c.oneLine {
				if r, ok := box(text); !ok || r.H > 24 {
					t.Errorf("%q is %v high, more than one line; texts %q", text, r.H, tt.Texts())
				}
			}
		})
	}
}

func quote(s string) string { return "“" + s + "”" }

func TestNearColorsDifferByLessThanThree(t *testing.T) {
	black, white := color.RGBA{0, 0, 0, 255}, color.RGBA{255, 255, 255, 255}
	if near(black, white) || near(white, black) || !near(black, color.RGBA{2, 1, 0, 255}) || near(black, color.RGBA{3, 0, 0, 255}) {
		t.Error("near is wrong")
	}
}

// near reports whether two colors differ by less than 3 in each channel.
func near(a, b color.RGBA) bool {
	d := func(x, y uint8) bool { return max(x, y)-min(x, y) < 3 }
	return d(a.R, b.R) && d(a.G, b.G) && d(a.B, b.B)
}

// longJSONCard is a JSON card with long keys and a long string, which the
// view must wrap.
func longJSONCard() ClipCard {
	c := jsonCard()
	j := *c.JSON
	long := `"` + strings.Repeat("abcdefgh", 40) + `"`
	j.Keys = nil
	for i := range 12 {
		j.Keys = append(j.Keys, JSONKey{Name: strings.Repeat("k", 10+i), Hint: "{12}"})
	}
	j.Minified = `{"a":[` + long + `,true]}`
	j.Lines = append([]JSONLine{}, j.Lines...)
	j.Lines[2] = JSONLine{Indent: 2, Tokens: []JSONToken{{JSONString, long}, {JSONPunctuation, ","}}}
	c.JSON = &j
	return c
}
