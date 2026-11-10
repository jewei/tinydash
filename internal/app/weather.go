package app

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/jewei/tinydash/internal/feature/weather"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// weatherRetry: after a weather download starts, the next one for the
// same city waits this long unless forced, so a failing one does not run
// at each show.
const weatherRetry = 10 * time.Minute

// openMeteo holds the two services a weather download asks: the city name
// goes to geocoding, and the place's coordinates to forecast.
type openMeteo struct{ geocoding, forecast string }

var openMeteoAPI = openMeteo{
	geocoding: "https://geocoding-api.open-meteo.com/v1/search",
	forecast:  "https://api.open-meteo.com/v1/forecast",
}

// weatherFailure is how the latest download for a city failed.
type weatherFailure struct {
	// notFound: no place has the city's name. Else message says why.
	notFound bool
	message  string
}

// weatherAttempt is the latest weather download: its city, when it
// started, whether it still runs, and how it failed.
type weatherAttempt struct {
	// number counts downloads, so only the latest one may report, even
	// when an older one was for the same city (A, then B, then A again).
	number  uint64
	city    string
	started time.Time
	busy    bool
	failure *weatherFailure
}

// start begins a download for city at now and returns its number, or
// false when none is due: one for the city runs, or, unless forced, the
// weather is fresh or the last try for the city was under weatherRetry
// ago. A failure for the same city stays shown until the new download
// ends.
func (w *weatherAttempt) start(city string, fresh, force bool, now time.Time) (uint64, bool) {
	same := weather.SameCity(w.city, city)
	waiting := !w.started.IsZero() && now.Sub(w.started) < weatherRetry
	if same && w.busy || !force && (fresh || same && waiting) {
		return 0, false
	}
	var failure *weatherFailure
	if same {
		failure = w.failure
	}
	*w = weatherAttempt{number: w.number + 1, city: city, started: now, busy: true, failure: failure}
	return w.number, true
}

// finish ends download number and reports whether to keep its result.
// Only the latest download keeps it, and only while its city is still the
// one in the settings: changing back to a city with fresh weather starts
// no download, so the one still running may be for a city the user left.
// A dropped download does not count as a recent try, so changing to its
// city again downloads at once.
func (w *weatherAttempt) finish(number uint64, wantedCity string) bool {
	if w.number != number {
		return false
	}
	w.busy = false
	keep := weather.SameCity(w.city, wantedCity)
	if !keep {
		w.started = time.Time{}
	}
	return keep
}

// failureFor is how the latest download for city failed, or nil.
func (w *weatherAttempt) failureFor(city string) *weatherFailure {
	if !weather.SameCity(w.city, city) {
		return nil
	}
	return w.failure
}

// weatherState is the app's weather download. mu also covers the store of
// a result, so a refresh that starts meanwhile sees it as fresh. The
// writer and the main thread never take mu.
type weatherState struct {
	mu      sync.Mutex
	attempt weatherAttempt
}

func (s *weatherState) failureFor(city string) *weatherFailure {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.attempt.failureFor(city)
}

// refreshWeather downloads the weather while the widget is on with a city
// whose weather is missing or old. force skips the retry delay and the age
// check, for Refresh. Only the latest download keeps its result, and only
// while its city is still the one in the settings.
func (a *App) refreshWeather(force bool) {
	now := time.Now()
	a.weather.mu.Lock()
	s := a.engine.Snapshot(now)
	city := strings.TrimSpace(s.Settings.WeatherCity)
	if !s.Settings.ShowWeather || city == "" {
		a.weather.mu.Unlock()
		return
	}
	fresh := s.Weather != nil && s.Weather.IsFor(city) && !s.Weather.IsStale(now)
	number, due := a.weather.attempt.start(city, fresh, force, now)
	a.weather.mu.Unlock()
	if !due {
		return
	}
	found, failure := fetchWeather(a.ctx, http.DefaultClient, openMeteoAPI, city, now)
	a.weather.mu.Lock()
	if !a.weather.attempt.finish(number, a.engine.Snapshot(time.Now()).Settings.WeatherCity) {
		a.weather.mu.Unlock()
		return
	}
	a.weather.attempt.failure = failure
	if failure == nil {
		if err := <-a.engine.SetWeather(found); err != nil {
			slog.Warn("weather", "error", err)
		}
	}
	a.weather.mu.Unlock()
	if failure != nil {
		// Nothing in the data changed, so the widget loads again here
		// to say why.
		a.panel.Update(a.model.Stale)
	}
}

// fetchWeather finds city and downloads its weather at now, in up to two
// requests of up to ten seconds each. The failure is nil on success.
func fetchWeather(ctx context.Context, client *http.Client, api openMeteo, city string, now time.Time) (weather.Weather, *weatherFailure) {
	failed := func(err error) (weather.Weather, *weatherFailure) {
		slog.Warn("weather", "error", err)
		// Parse errors are sentences for the user already.
		message := err.Error()
		var download userMessage
		if errors.As(err, &download) {
			message = download.text
		}
		return weather.Weather{}, &weatherFailure{message: message}
	}
	var place weather.Place
	var found bool
	search := api.geocoding + "?" + url.Values{"name": {city}, "count": {"1"}, "format": {"json"}}.Encode()
	err := download(ctx, client, search, func(r io.Reader) (err error) {
		place, found, err = weather.ParsePlace(r)
		return err
	})
	if err != nil {
		return failed(err)
	}
	if !found {
		return weather.Weather{}, &weatherFailure{notFound: true}
	}
	forecast := api.forecast + "?" + url.Values{
		"latitude":      {strconv.FormatFloat(place.Latitude, 'f', -1, 64)},
		"longitude":     {strconv.FormatFloat(place.Longitude, 'f', -1, 64)},
		"current":       {"temperature_2m,weather_code,is_day"},
		"daily":         {"temperature_2m_max,temperature_2m_min,precipitation_probability_max"},
		"timezone":      {"auto"},
		"forecast_days": {"1"},
	}.Encode()
	var w weather.Weather
	err = download(ctx, client, forecast, func(r io.Reader) (err error) {
		w, err = weather.ParseForecast(r, city, place, now)
		return err
	})
	if err != nil {
		return failed(err)
	}
	return w, nil
}

// userMessage is a download error as the widget words it; err is the
// detail for the log.
type userMessage struct {
	text string
	err  error
}

func (m userMessage) Error() string { return fmt.Sprintf("%s (%v)", m.text, m.err) }

// download gets address and hands its body to read, within ten seconds.
// Each feature limits how much of the body it reads.
func download(ctx context.Context, client *http.Client, address string, read func(io.Reader) error) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, address, nil)
	if err != nil {
		return err
	}
	response, err := client.Do(request)
	if err != nil {
		return userMessage{"Open-Meteo did not answer. Check your internet connection.", err}
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return userMessage{fmt.Sprintf("Open-Meteo answered with error %d. Try again later.", response.StatusCode), errors.New(response.Status)}
	}
	return read(response.Body)
}

// weatherCard is the weather widget for the settings: the saved weather
// while it is for the city, or how getting it goes. zone is the computer's
// time zone now.
func weatherCard(s settings.Settings, saved *weather.Weather, failure *weatherFailure, zone *time.Location) *launcher.Weather {
	city := strings.TrimSpace(s.WeatherCity)
	switch {
	case city == "":
		return &launcher.Weather{State: launcher.WeatherNoCity}
	case saved != nil && saved.IsFor(city):
		fahrenheit := s.TemperatureUnit == settings.Fahrenheit
		condition, icon := weather.Describe(saved.Code, saved.IsDay)
		return &launcher.Weather{
			State:       launcher.WeatherReady,
			City:        city,
			Place:       saved.Place,
			Temperature: weather.Degrees(saved.Temperature, fahrenheit),
			High:        weather.Degrees(saved.High, fahrenheit),
			Low:         weather.Degrees(saved.Low, fahrenheit),
			Condition:   condition,
			Icon:        string(icon),
			RainChance:  saved.RainChance,
			UpdatedAt:   time.Unix(saved.FetchedAt, 0).In(zone),
			Offline:     failure != nil,
		}
	case failure == nil:
		return &launcher.Weather{State: launcher.WeatherLoading, City: city}
	case failure.notFound:
		return &launcher.Weather{State: launcher.WeatherNotFound, City: city}
	}
	return &launcher.Weather{State: launcher.WeatherFailed, City: city, Message: failure.message}
}
