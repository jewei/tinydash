package launcher

import (
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

func TestShowsTheWeatherOfTheCity(t *testing.T) {
	f := newFake(always())
	rain := uint8(40)
	updated := time.Date(2026, 11, 10, 14, 5, 0, 0, time.FixedZone("UTC+8", 8*3600))
	f.widgets = Widgets{Weather: &Weather{
		State: WeatherReady, City: "singapore", Place: "Singapore", Temperature: 31, High: 33, Low: 26,
		Condition: "Partly cloudy", Icon: "partlyDay", RainChance: &rain, UpdatedAt: updated,
	}, ClipCards: true}
	_, tt := showLauncher(t, f)
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Weather"); !ok || !hasPane(tt) {
		t.Fatalf("no weather; texts %q", tt.Texts())
	}
	// The weather fills the pane, so the hint of an empty clipboard card
	// does not show.
	if tt.HasText("Copy a color, a Unix time, or JSON to see it here.") {
		t.Error("the empty-pane hint shows beside the weather")
	}
	for _, text := range []string{"Singapore", "31°", "Partly cloudy", "H 33° · L 26° · Rain 40%", "Updated 14:05"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
}

// Offline weather says how old it is, and the age moves on with no load
// in between.
func TestOfflineWeatherSaysHowOldItIs(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Weather: &Weather{State: WeatherReady, City: "Oslo", Place: "Oslo", High: 3, Low: -2, Condition: "Snow", Icon: "snow", Offline: true}}
	m := New(f)
	tt := ui.NewTester(m.View, 880, 560)
	f.widgets.Weather.UpdatedAt = tt.Now().Add(-59*time.Minute - 30*time.Second)
	m.Show("")
	tt.Frame()
	if !tt.HasText("Offline · updated 59 min ago") || !tt.HasText("H 3° · L -2°") || tt.HasText("Rain") {
		t.Fatalf("texts %q", tt.Texts())
	}
	tt.Advance(30 * time.Second)
	if !tt.HasText("Offline · updated 1 h ago") {
		t.Errorf("texts %q", tt.Texts())
	}
	if f.widgetLoads != 1 {
		t.Errorf("loaded the widgets %d times", f.widgetLoads)
	}
}

func TestSaysWhatKeepsTheWeatherFromShowing(t *testing.T) {
	for _, c := range []struct {
		weather  Weather
		texts    []string
		settings bool
	}{
		{Weather{State: WeatherNoCity}, []string{"Set a city in Settings."}, true},
		{Weather{State: WeatherLoading, City: "Oslo"}, []string{"Oslo", "Getting the weather…"}, false},
		{Weather{State: WeatherNotFound, City: "Atlantis"}, []string{"No place is named “Atlantis”."}, true},
		{Weather{State: WeatherFailed, City: "Oslo", Message: "Check your internet connection."}, []string{"Oslo", "Could not get the weather. Check your internet connection."}, false},
	} {
		f := newFake(always())
		f.widgets = Widgets{Weather: &c.weather}
		_, tt := showLauncher(t, f)
		for _, text := range c.texts {
			if !tt.HasText(text) {
				t.Errorf("%v: no %q in %q", c.weather.State, text, tt.Texts())
			}
		}
		if _, ok := tt.AccessByRole(ui.RoleButton, "Open Settings"); ok != c.settings {
			t.Errorf("%v: Open Settings %v", c.weather.State, ok)
		}
		if c.settings {
			pressButton(t, tt, "Open Settings")
			if len(f.runs) != 1 || f.runs[0] != (ran{core.OpenSettings{}, ""}) {
				t.Errorf("%v: runs %+v", c.weather.State, f.runs)
			}
		}
	}
}

func TestNamesTheWeatherInRefreshWhenItIsOn(t *testing.T) {
	for _, c := range []struct {
		rates bool
		want  string
	}{
		{true, "Refresh Apps, Files, Rates, and Weather"},
		{false, "Refresh Apps, Files, and Weather"},
	} {
		f := newFake(always(safari))
		f.settings.CurrencyRates, f.settings.Weather = c.rates, true
		_, tt := open(t, f)
		tt.Key(ui.Cmd, ui.KeyK)
		if _, ok := tt.AccessByRole(ui.RoleOption, c.want); !ok {
			t.Errorf("no %q in %q", c.want, tt.Texts())
		}
	}
}

func TestSaysHowLongAgoInWords(t *testing.T) {
	now := time.Date(2026, 11, 10, 12, 0, 0, 0, time.UTC)
	for before, want := range map[time.Duration]string{
		-time.Hour:       "just now",
		30 * time.Second: "just now",
		5 * time.Minute:  "5 min ago",
		3 * time.Hour:    "3 h ago",
		30 * time.Hour:   "1 day ago",
		50 * time.Hour:   "2 days ago",
	} {
		if got := ago(now.Add(-before), now); got != want {
			t.Errorf("ago(%v) = %q, want %q", before, got, want)
		}
	}
}
