package weather

import (
	"encoding/json"
	"strings"
	"testing"
	"time"
)

const forecastJSON = `{"latitude":1.29,"longitude":103.85,
	"current":{"time":"2026-10-07T15:00","temperature_2m":31.4,"weather_code":2,"is_day":1},
	"daily":{"time":["2026-10-07"],"temperature_2m_max":[33.2],"temperature_2m_min":[25.6],
	"precipitation_probability_max":[40]}}`

var singaporePlace = Place{Name: "Singapore", Latitude: 1.28967, Longitude: 103.85007}

func singapore(t *testing.T) Weather {
	t.Helper()
	w, err := ParseForecast(strings.NewReader(forecastJSON), " singapore ", singaporePlace, time.Unix(100, 0))
	if err != nil {
		t.Fatal(err)
	}
	return w
}

func TestReadsAPlace(t *testing.T) {
	found := `{"results":[{"id":1880252,"name":"Singapore","latitude":1.28967,"longitude":103.85007,"country":"Singapore"}]}`
	place, ok, err := ParsePlace(strings.NewReader(found))
	if err != nil || !ok || place != singaporePlace {
		t.Errorf("place %+v, %v, %v", place, ok, err)
	}
	for _, none := range []string{`{"generationtime_ms":0.5}`, `{"results":[{"name":"Nowhere","latitude":91,"longitude":0}]}`} {
		if _, ok, err := ParsePlace(strings.NewReader(none)); ok || err != nil {
			t.Errorf("%s: found %v, %v", none, ok, err)
		}
	}
	for _, bad := range []string{`<html>`, `{"results":[{"name":"Oslo"}]}`} {
		if _, _, err := ParsePlace(strings.NewReader(bad)); err == nil {
			t.Errorf("%s: no error", bad)
		}
	}
}

func TestReadsAForecast(t *testing.T) {
	w := singapore(t)
	if w.City != "singapore" || w.Place != "Singapore" || w.Temperature != 31.4 || w.High != 33.2 || w.Low != 25.6 ||
		w.Code != 2 || !w.IsDay || w.FetchedAt != 100 || w.RainChance == nil || *w.RainChance != 40 {
		t.Errorf("weather %+v", w)
	}
	if !w.IsFor("Singapore") || !w.IsFor("  SINGAPORE ") || w.IsFor("Tokyo") {
		t.Error("IsFor")
	}

	noRain := strings.Replace(forecastJSON, "[40]", "[null]", 1)
	if w, err := ParseForecast(strings.NewReader(noRain), "x", Place{}, time.Unix(0, 0)); err != nil || w.RainChance != nil {
		t.Errorf("no rain chance: %+v, %v", w, err)
	}
	over := strings.Replace(forecastJSON, "[40]", "[100.4]", 1)
	if w, err := ParseForecast(strings.NewReader(over), "x", Place{}, time.Unix(0, 0)); err != nil || *w.RainChance != 100 {
		t.Errorf("rain chance over 100: %+v, %v", w, err)
	}
	for _, bad := range []string{
		strings.Replace(forecastJSON, "[33.2]", "[]", 1),
		strings.Replace(forecastJSON, "[25.6]", "[null]", 1),
		strings.Replace(forecastJSON, `"temperature_2m":31.4`, `"temperature_2m":null`, 1),
		strings.Replace(forecastJSON, `"weather_code":2`, `"weather_code":300`, 1),
		`{"current":{"temperature_2m":1,"weather_code":0,"is_day":1}}`,
		"<html>",
	} {
		if _, err := ParseForecast(strings.NewReader(bad), "x", Place{}, time.Unix(0, 0)); err == nil {
			t.Errorf("%s: no error", bad)
		}
	}
}

func TestReadsNoMoreThanTheLimit(t *testing.T) {
	padded := `{"results":[` + strings.Repeat(" ", MaxResponseBytes) + `]}`
	if _, _, err := ParsePlace(strings.NewReader(padded)); err == nil {
		t.Error("read past the limit")
	}
}

func TestIsStaleAfterHalfAnHourOrFromAClockThatWasAhead(t *testing.T) {
	w := Weather{FetchedAt: 100}
	at := func(seconds int64) time.Time { return time.Unix(seconds, 0) }
	if w.IsStale(at(100 + 30*60 - 1)) {
		t.Error("stale before 30 minutes")
	}
	if !w.IsStale(at(100 + 30*60)) {
		t.Error("fresh after 30 minutes")
	}
	if !w.IsStale(at(99)) {
		t.Error("weather from a clock that was ahead is fresh")
	}
}

// The cached weather keeps the Rust app's JSON, so either app reads it.
func TestKeepsTheRustCacheFormat(t *testing.T) {
	rust := `{"city":"singapore","place":"Singapore","temperature":31.4,"high":33.2,"low":25.6,"rain_chance":40,"code":2,"is_day":true,"fetched_at":100}`
	var w Weather
	if err := json.Unmarshal([]byte(rust), &w); err != nil {
		t.Fatal(err)
	}
	if got := singapore(t); w.City != got.City || *w.RainChance != *got.RainChance || w.FetchedAt != got.FetchedAt {
		t.Errorf("read %+v", w)
	}
	data, err := json.Marshal(w)
	if err != nil || string(data) != rust {
		t.Errorf("wrote %s, %v", data, err)
	}
}

func TestShowsWholeDegreesInTheChosenUnit(t *testing.T) {
	w := singapore(t)
	if got := [3]int{Degrees(w.Temperature, true), Degrees(w.High, true), Degrees(w.Low, false)}; got != [3]int{89, 92, 26} {
		t.Errorf("degrees %v", got)
	}
}

// The words and icon of each WMO code, as the Rust app gave them. Codes
// that Open-Meteo does not document read as Unknown under a cloud.
func TestDescribesEveryWeatherCodeAsTheRustAppDid(t *testing.T) {
	type answer struct {
		condition string
		icon      Icon
	}
	byCode := map[uint8]answer{
		3:  {"Cloudy", Cloudy},
		45: {"Fog", Fog}, 48: {"Fog", Fog},
		51: {"Drizzle", Rain}, 53: {"Drizzle", Rain}, 55: {"Drizzle", Rain},
		56: {"Freezing drizzle", Rain}, 57: {"Freezing drizzle", Rain},
		61: {"Light rain", Rain}, 63: {"Rain", Rain}, 65: {"Heavy rain", Rain},
		66: {"Freezing rain", Rain}, 67: {"Freezing rain", Rain},
		71: {"Light snow", Snow}, 73: {"Snow", Snow}, 75: {"Heavy snow", Snow}, 77: {"Snow", Snow},
		80: {"Rain showers", Rain}, 81: {"Rain showers", Rain}, 82: {"Rain showers", Rain},
		85: {"Snow showers", Snow}, 86: {"Snow showers", Snow},
		95: {"Thunderstorm", Thunder},
		96: {"Thunderstorm with hail", Thunder}, 99: {"Thunderstorm with hail", Thunder},
	}
	for _, isDay := range []bool{true, false} {
		want := map[uint8]answer{0: {"Clear", ClearDay}, 1: {"Mostly clear", PartlyDay}, 2: {"Partly cloudy", PartlyDay}}
		if !isDay {
			want = map[uint8]answer{0: {"Clear", ClearNight}, 1: {"Mostly clear", PartlyNight}, 2: {"Partly cloudy", PartlyNight}}
		}
		for code, a := range byCode {
			want[code] = a
		}
		for code := 0; code <= 255; code++ {
			w, ok := want[uint8(code)]
			if !ok {
				w = answer{"Unknown", Cloudy}
			}
			if condition, icon := Describe(uint8(code), isDay); condition != w.condition || icon != w.icon {
				t.Errorf("Describe(%d, %v) = %q, %q, want %q, %q", code, isDay, condition, icon, w.condition, w.icon)
			}
		}
	}
}
