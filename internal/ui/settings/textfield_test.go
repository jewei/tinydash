package settings

import (
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
