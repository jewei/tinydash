package engine

import (
	"context"
	"testing"

	"github.com/jewei/tinydash/internal/feature/weather"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// Weather is kept only while it is for the city in the settings: a
// download that ends after the user chose another city changes nothing.
func TestKeepsWeatherOnlyForTheCityInTheSettings(t *testing.T) {
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.Settings.ShowWeather = true
		first.Settings.WeatherCity = "Oslo"
	})
	ctx := context.Background()
	oslo := weather.Weather{City: "oslo", Place: "Oslo", FetchedAt: 100}
	if err := <-w.engine.SetWeather(weather.Weather{City: "Bergen", Place: "Bergen"}); err != nil {
		t.Fatal(err)
	}
	if _, ok, _ := w.db.Weather(ctx); ok || w.snapshot().Weather != nil {
		t.Fatal("kept the weather of another city")
	}

	if err := <-w.engine.SetWeather(oslo); err != nil {
		t.Fatal(err)
	}
	saved, ok, _ := w.db.Weather(ctx)
	if got := w.snapshot().Weather; !ok || got == nil || got.Place != "Oslo" || saved == "" {
		t.Fatalf("weather %+v, saved %q", got, saved)
	}
}
