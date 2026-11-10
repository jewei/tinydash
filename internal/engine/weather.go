package engine

import (
	"context"
	"encoding/json"
	"log/slog"

	"github.com/jewei/tinydash/internal/feature/weather"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// SetWeather saves a weather download and publishes it, unless the user
// chose another city while it downloaded. The check runs on the writer,
// after any settings change sent before it, so a download for a city the
// user left never replaces the weather shown. A download kept while the
// widget is off shows when it is on again, as in the Rust app. A save
// that fails still publishes: the weather shows until the next start.
func (e *Engine) SetWeather(w weather.Weather) <-chan error {
	return e.Change(func(next *search.Snapshot, db *store.Store) error {
		if !w.IsFor(next.Settings.WeatherCity) {
			return errUnchanged
		}
		data, err := json.Marshal(w)
		if err != nil {
			return err
		}
		if err := db.SaveWeather(context.Background(), string(data)); err != nil {
			slog.Warn("Could not save the weather", "error", err)
		}
		next.Weather = &w
		return nil
	})
}
