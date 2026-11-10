package app

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/feature/weather"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

var weatherNow = time.Date(2026, 11, 10, 6, 0, 0, 0, time.UTC)

func TestOnlyTheLatestWeatherDownloadReports(t *testing.T) {
	var a weatherAttempt
	firstOslo, _ := a.start("Oslo", false, false, weatherNow)
	bergen, _ := a.start("Bergen", false, false, weatherNow)
	secondOslo, ok := a.start("Oslo", false, false, weatherNow)
	if !ok || firstOslo == secondOslo || bergen == secondOslo || !a.busy {
		t.Fatalf("numbers %d %d %d, %+v", firstOslo, bergen, secondOslo, a)
	}
	if a.finish(firstOslo, "Oslo") || !a.busy {
		t.Error("an older download for the same city reported")
	}
	if !a.finish(secondOslo, " oslo ") || a.busy || a.started.IsZero() {
		t.Errorf("the latest download did not report: %+v", a)
	}
}

// Changing the city and back while a download runs drops it, and the
// next try for its city does not wait.
func TestADownloadForACityTheUserLeftIsDropped(t *testing.T) {
	var a weatherAttempt
	oslo, _ := a.start("Oslo", false, false, weatherNow)
	if a.finish(oslo, "Bergen") || a.busy || !a.started.IsZero() {
		t.Errorf("kept a download for a city the user left: %+v", a)
	}
	if _, ok := a.start("Oslo", false, false, weatherNow.Add(time.Minute)); !ok {
		t.Error("the dropped city waits for the retry")
	}

	// The writer can still drop a kept download, when the city changed
	// after finish looked; changing back must not wait either.
	bergen, _ := a.start("Bergen", false, false, weatherNow.Add(2*time.Minute))
	if !a.finish(bergen, "Bergen") {
		t.Fatal("dropped the latest download")
	}
	a.dropped()
	if _, ok := a.start("Bergen", false, false, weatherNow.Add(3*time.Minute)); !ok {
		t.Error("a download that the writer dropped waits for the retry")
	}
}

func TestWaitsBeforeTryingTheSameCityAgain(t *testing.T) {
	var a weatherAttempt
	number, _ := a.start("Oslo", false, false, weatherNow)
	if _, ok := a.start("oslo", false, true, weatherNow); ok {
		t.Error("a forced download started while one for the city runs")
	}
	a.finish(number, "Oslo")
	a.failure = &weatherFailure{message: "Open-Meteo did not answer."}
	if _, ok := a.start("Oslo", false, false, weatherNow.Add(weatherRetry-time.Second)); ok {
		t.Error("retried before weatherRetry")
	}
	if _, ok := a.start("Oslo", true, false, weatherNow.Add(weatherRetry)); ok {
		t.Error("downloaded fresh weather")
	}
	if _, ok := a.start("Oslo", false, true, weatherNow.Add(time.Second)); !ok {
		t.Error("a forced download waited")
	}
	if a.failureFor("OSLO") == nil {
		t.Error("a new download for the city hid its failure")
	}
	if _, ok := a.start("Bergen", false, false, weatherNow.Add(2*time.Second)); !ok || a.failureFor("Oslo") != nil || a.failureFor("Bergen") != nil {
		t.Errorf("another city: %+v", a)
	}
}

func TestADownloadEndsByStoringOrSayingWhy(t *testing.T) {
	var s weatherState
	city := func(name string) func() string { return func() string { return name } }
	stored := 0
	store := func(err error) func() error { return func() error { stored++; return err } }

	number, _ := s.attempt.start("Oslo", false, false, weatherNow)
	if s.end(number, city("Oslo"), nil, store(nil)) || stored != 1 || s.failureFor("Oslo") != nil {
		t.Errorf("a success: stored %d, %+v", stored, s.attempt)
	}

	// The writer dropped the weather, as the city changed after finish
	// looked: changing back downloads at once.
	number, _ = s.attempt.start("Bergen", false, false, weatherNow)
	s.end(number, city("Bergen"), nil, store(engine.ErrOtherCity))
	if _, ok := s.attempt.start("Bergen", false, false, weatherNow.Add(time.Minute)); !ok {
		t.Error("a download that the writer dropped waits for the retry")
	}

	number, _ = s.attempt.start("Ipoh", false, false, weatherNow)
	failure := &weatherFailure{notFound: true}
	if !s.end(number, city("Ipoh"), failure, store(nil)) || s.failureFor("ipoh") != failure || stored != 2 {
		t.Errorf("a failure: stored %d, %+v", stored, s.attempt)
	}

	// A download for a city the user left stores nothing and says nothing.
	number, _ = s.attempt.start("Oslo", false, false, weatherNow)
	if s.end(number, city("Ipoh"), nil, store(nil)) || stored != 2 {
		t.Errorf("a city the user left: stored %d", stored)
	}
}

func TestTheWeatherCardShowsOnlyTheCityInTheSettings(t *testing.T) {
	s := settings.Default()
	s.ShowWeather = true
	zone := time.FixedZone("UTC+8", 8*3600)
	rain := uint8(40)
	saved := &weather.Weather{City: "oslo", Place: "Oslo", Temperature: 31.4, High: 33.2, Low: 25.6, RainChance: &rain, Code: 2, IsDay: true, FetchedAt: weatherNow.Unix()}

	if got := weatherCard(s, saved, nil, zone); got.State != launcher.WeatherNoCity {
		t.Errorf("no city: %+v", got)
	}
	s.WeatherCity = " Oslo "
	s.TemperatureUnit = settings.Fahrenheit
	want := launcher.Weather{
		State: launcher.WeatherReady, City: "Oslo", Place: "Oslo", Temperature: 89, High: 92, Low: 78,
		Condition: "Partly cloudy", Icon: "partlyDay", RainChance: &rain, UpdatedAt: weatherNow.In(zone),
	}
	got := weatherCard(s, saved, nil, zone)
	if *got != want || got.UpdatedAt.Format("15:04") != "14:00" {
		t.Errorf("ready: %+v", got)
	}
	if got := weatherCard(s, saved, &weatherFailure{message: "x"}, zone); !got.Offline || got.State != launcher.WeatherReady {
		t.Errorf("offline: %+v", got)
	}

	s.WeatherCity = "Bergen"
	for _, c := range []struct {
		failure *weatherFailure
		want    launcher.Weather
	}{
		{nil, launcher.Weather{State: launcher.WeatherLoading, City: "Bergen"}},
		{&weatherFailure{notFound: true}, launcher.Weather{State: launcher.WeatherNotFound, City: "Bergen"}},
		{&weatherFailure{message: "Try again later."}, launcher.Weather{State: launcher.WeatherFailed, City: "Bergen", Message: "Try again later."}},
	} {
		if got := weatherCard(s, saved, c.failure, zone); *got != c.want {
			t.Errorf("%+v: %+v", c.failure, got)
		}
	}
}

// openMeteoFake answers as Open-Meteo does, with search for the
// geocoding service and forecast for the forecast service.
func openMeteoFake(t *testing.T, search, forecast func(http.ResponseWriter, *http.Request)) openMeteo {
	t.Helper()
	mux := http.NewServeMux()
	mux.HandleFunc("/v1/search", search)
	mux.HandleFunc("/v1/forecast", forecast)
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	return openMeteo{geocoding: server.URL + "/v1/search", forecast: server.URL + "/v1/forecast"}
}

func reply(body string) func(http.ResponseWriter, *http.Request) {
	return func(w http.ResponseWriter, _ *http.Request) { w.Write([]byte(body)) }
}

func TestFetchesTheWeatherOfTheFirstPlaceFound(t *testing.T) {
	var asked []string
	api := openMeteoFake(t, func(w http.ResponseWriter, r *http.Request) {
		asked = append(asked, r.URL.RawQuery)
		w.Write([]byte(`{"results":[{"name":"Kuala Lumpur","latitude":3.1412,"longitude":101.68653}]}`))
	}, func(w http.ResponseWriter, r *http.Request) {
		asked = append(asked, r.URL.RawQuery)
		w.Write([]byte(`{"current":{"temperature_2m":27,"weather_code":61,"is_day":0},
			"daily":{"temperature_2m_max":[32],"temperature_2m_min":[24],"precipitation_probability_max":[null]}}`))
	})
	got, failure := fetchWeather(context.Background(), http.DefaultClient, api, "kuala lumpur", weatherNow)
	if failure != nil || got.City != "kuala lumpur" || got.Place != "Kuala Lumpur" || got.Code != 61 || got.IsDay || got.FetchedAt != weatherNow.Unix() {
		t.Errorf("%+v, %+v", got, failure)
	}
	want := []string{
		"count=1&format=json&name=kuala+lumpur",
		"current=temperature_2m%2Cweather_code%2Cis_day&daily=temperature_2m_max%2Ctemperature_2m_min%2Cprecipitation_probability_max&forecast_days=1&latitude=3.1412&longitude=101.68653&timezone=auto",
	}
	if len(asked) != 2 || asked[0] != want[0] || asked[1] != want[1] {
		t.Errorf("asked %q", asked)
	}
}

func TestSaysWhyTheWeatherDidNotCome(t *testing.T) {
	never := func(w http.ResponseWriter, _ *http.Request) { t.Error("asked for a forecast") }
	notFound := openMeteoFake(t, reply(`{"generationtime_ms":0.4}`), never)
	if _, failure := fetchWeather(context.Background(), http.DefaultClient, notFound, "Atlantis", weatherNow); failure == nil || !failure.notFound {
		t.Errorf("unknown city: %+v", failure)
	}

	limited := openMeteoFake(t, func(w http.ResponseWriter, _ *http.Request) {
		http.Error(w, "Too many requests", http.StatusTooManyRequests)
	}, never)
	if _, failure := fetchWeather(context.Background(), http.DefaultClient, limited, "Oslo", weatherNow); failure == nil || failure.message != "Open-Meteo answered with error 429. Try again later." {
		t.Errorf("rate limited: %+v", failure)
	}

	garbled := openMeteoFake(t, reply(`{"results":[{"name":"Oslo","latitude":59.9,"longitude":10.7}]}`), reply("<html>"))
	if _, failure := fetchWeather(context.Background(), http.DefaultClient, garbled, "Oslo", weatherNow); failure == nil || failure.message != "The weather service sent unexpected data. Try again later." {
		t.Errorf("garbled forecast: %+v", failure)
	}

	// The time runs out while the body comes.
	ctx, cancel := context.WithCancel(context.Background())
	slow := openMeteoFake(t, func(w http.ResponseWriter, _ *http.Request) {
		w.Write([]byte(`{"results":[`))
		w.(http.Flusher).Flush()
		// The client has the headers by now and waits for the rest.
		time.Sleep(100 * time.Millisecond)
		cancel()
		time.Sleep(100 * time.Millisecond)
	}, never)
	if _, failure := fetchWeather(ctx, http.DefaultClient, slow, "Oslo", weatherNow); failure == nil || !strings.Contains(failure.message, "did not answer in time") {
		t.Errorf("slow body: %+v", failure)
	}

	// The connection breaks while the body comes.
	broken := openMeteoFake(t, func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Length", "1000")
		w.Write([]byte(`{"results":[`))
	}, never)
	if _, failure := fetchWeather(context.Background(), http.DefaultClient, broken, "Oslo", weatherNow); failure == nil || !strings.Contains(failure.message, "The connection to Open-Meteo broke.") {
		t.Errorf("broken body: %+v", failure)
	}

	offline := openMeteo{geocoding: "http://127.0.0.1:1/v1/search", forecast: "http://127.0.0.1:1/v1/forecast"}
	if _, failure := fetchWeather(context.Background(), http.DefaultClient, offline, "Oslo", weatherNow); failure == nil || !strings.Contains(failure.message, "Check your internet connection.") {
		t.Errorf("offline: %+v", failure)
	}
}
