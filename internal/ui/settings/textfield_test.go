package settings

import (
	"errors"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/settings"
)

func TestSetsTheWeatherCityAndUnit(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, widgets)
	flip(t, tt, "Weather")
	if !f.last(t).ShowWeather {
		t.Fatal("the weather switch saved nothing")
	}
	typeInto(t, tt, "Weather city", "  Kuala Lumpur ", true)
	if got := f.last(t).WeatherCity; got != "Kuala Lumpur" {
		t.Errorf("city %q", got)
	}
	if info, _ := tt.AccessByRole(ui.RoleTextField, "Weather city"); info.Value != "Kuala Lumpur" {
		t.Errorf("field %q", info.Value)
	}
	// The same city again saves nothing.
	typeInto(t, tt, "Weather city", "Kuala Lumpur", true)
	f.wantUpdates(t, 2)

	// Leaving the field saves it too.
	typeInto(t, tt, "Weather city", "Ipoh", false)
	press(t, tt, ui.RolePopUpButton, "Temperature")
	press(t, tt, ui.RoleOption, "Fahrenheit (°F)")
	if s := f.last(t); s.WeatherCity != "Ipoh" || s.TemperatureUnit != settings.Fahrenheit {
		t.Errorf("city %q, unit %q", s.WeatherCity, s.TemperatureUnit)
	}
}

// Going back to the saved city while another city is still saving saves
// the city typed last.
func TestTheLastCityTypedWinsOverASaveOnItsWay(t *testing.T) {
	f := newFake()
	f.saved.WeatherCity = "Oslo"
	f.hold = true
	_, tt := open(t, f, widgets)
	typeInto(t, tt, "Weather city", "Ipoh", true)
	typeInto(t, tt, "Weather city", "Oslo", true)
	f.release(t, tt)
	f.release(t, tt)
	if got := f.last(t).WeatherCity; got != "Oslo" {
		t.Errorf("city %q", got)
	}
	if info, _ := tt.AccessByRole(ui.RoleTextField, "Weather city"); info.Value != "Oslo" {
		t.Errorf("field %q", info.Value)
	}
}

// A city that could not be saved stays in the field, so it is not lost.
func TestKeepsACityThatCouldNotBeSaved(t *testing.T) {
	f := newFake()
	f.refuse = func(settings.Settings) error { return errors.New("Settings cannot be saved now.") }
	_, tt := open(t, f, widgets)
	typeInto(t, tt, "Weather city", " Ipoh ", true)
	tt.Frame()
	if info, _ := tt.AccessByRole(ui.RoleTextField, "Weather city"); info.Value != "Ipoh" || !tt.HasText("Not saved. Settings cannot be saved now.") {
		t.Errorf("field %q; texts %q", info.Value, tt.Texts())
	}
}
