// Package weather is the weather widget: current conditions for one city
// from Open-Meteo, which needs no account. The city name goes to its
// geocoding service, and the city's coordinates to its forecast service;
// nothing else does. The downloads are the app's job; this package reads
// and checks them.
package weather

import (
	"encoding/json"
	"errors"
	"io"
	"math"
	"strings"
	"time"
)

// MaxResponseBytes is the most of an Open-Meteo response that a parse
// reads.
const MaxResponseBytes = 64 * 1024

// MaxAge is how old weather may be before the launcher downloads it again.
const MaxAge = 30 * time.Minute

// Weather is a download, cached in tinydash.db for offline use. The JSON
// names are the ones that the Rust app saved, so a saved download loads in
// both.
type Weather struct {
	// City is the city as the setting names it, so a new city is
	// downloaded again.
	City string `json:"city"`
	// Place is the place Open-Meteo found, such as Singapore.
	Place string `json:"place"`
	// Temperature, High, and Low are in degrees Celsius.
	Temperature float64 `json:"temperature"`
	High        float64 `json:"high"`
	Low         float64 `json:"low"`
	// RainChance is today's chance of rain in percent, or nil when the
	// service has none.
	RainChance *uint8 `json:"rain_chance"`
	// Code is the WMO weather code.
	Code  uint8 `json:"code"`
	IsDay bool  `json:"is_day"`
	// FetchedAt is the download time in Unix seconds.
	FetchedAt int64 `json:"fetched_at"`
}

// SameCity reports whether two city settings name the same city: case and
// surrounding spaces do not count.
func SameCity(a, b string) bool {
	return strings.ToLower(strings.TrimSpace(a)) == strings.ToLower(strings.TrimSpace(b))
}

// IsFor reports whether the weather is that of the city setting.
func (w Weather) IsFor(city string) bool { return SameCity(w.City, city) }

// IsStale reports whether the weather is old at now. Weather saved in the
// future came from a clock that was ahead; it is stale, or it would stay
// fresh until the clock catches up.
func (w Weather) IsStale(now time.Time) bool {
	seconds := now.Unix()
	return seconds < w.FetchedAt || seconds-w.FetchedAt >= int64(MaxAge/time.Second)
}

// Place is where a geocoding search found the city.
type Place struct {
	Name                string
	Latitude, Longitude float64
}

var errUnexpected = errors.New("The weather service sent unexpected data.")

// ParsePlace reads a geocoding response: the first place found, or false
// when there is none.
func ParsePlace(r io.Reader) (Place, bool, error) {
	var response struct {
		Results []struct {
			Name      *string  `json:"name"`
			Latitude  *float64 `json:"latitude"`
			Longitude *float64 `json:"longitude"`
		} `json:"results"`
	}
	if err := decode(r, &response); err != nil {
		return Place{}, false, err
	}
	if len(response.Results) == 0 {
		return Place{}, false, nil
	}
	first := response.Results[0]
	if first.Name == nil || first.Latitude == nil || first.Longitude == nil {
		return Place{}, false, errUnexpected
	}
	lat, lon := *first.Latitude, *first.Longitude
	if lat < -90 || lat > 90 || lon < -180 || lon > 180 {
		return Place{}, false, nil
	}
	return Place{Name: *first.Name, Latitude: lat, Longitude: lon}, true, nil
}

// ParseForecast reads a forecast response for the city setting, as found
// at place, downloaded at now.
func ParseForecast(r io.Reader, city string, place Place, now time.Time) (Weather, error) {
	var response struct {
		Current *struct {
			Temperature *float64 `json:"temperature_2m"`
			Code        *uint8   `json:"weather_code"`
			IsDay       *uint8   `json:"is_day"`
		} `json:"current"`
		Daily *struct {
			High []*float64 `json:"temperature_2m_max"`
			Low  []*float64 `json:"temperature_2m_min"`
			Rain []*float64 `json:"precipitation_probability_max"`
		} `json:"daily"`
	}
	if err := decode(r, &response); err != nil {
		return Weather{}, err
	}
	current, daily := response.Current, response.Daily
	if current == nil || daily == nil || current.Temperature == nil || current.Code == nil || current.IsDay == nil {
		return Weather{}, errUnexpected
	}
	high, okHigh := today(daily.High)
	low, okLow := today(daily.Low)
	if !okHigh || !okLow {
		return Weather{}, errUnexpected
	}
	w := Weather{
		City:        strings.TrimSpace(city),
		Place:       place.Name,
		Temperature: *current.Temperature,
		High:        high,
		Low:         low,
		Code:        *current.Code,
		IsDay:       *current.IsDay != 0,
		FetchedAt:   now.Unix(),
	}
	if chance, ok := today(daily.Rain); ok {
		// A percentage, so the conversion loses at most the fraction.
		percent := uint8(math.Round(min(max(chance, 0), 100)))
		w.RainChance = &percent
	}
	return w, nil
}

// today is the first of a daily series, the value for today.
func today(values []*float64) (float64, bool) {
	if len(values) == 0 || values[0] == nil {
		return 0, false
	}
	return *values[0], true
}

func decode(r io.Reader, v any) error {
	if err := json.NewDecoder(io.LimitReader(r, MaxResponseBytes)).Decode(v); err != nil {
		return errUnexpected
	}
	return nil
}

// Degrees is a Celsius temperature in whole degrees, Fahrenheit when
// fahrenheit is set.
func Degrees(celsius float64, fahrenheit bool) int {
	if fahrenheit {
		celsius = celsius*9/5 + 32
	}
	return int(math.Round(celsius))
}

// Icon names the picture the widget draws for the weather.
type Icon string

const (
	ClearDay    Icon = "clearDay"
	ClearNight  Icon = "clearNight"
	PartlyDay   Icon = "partlyDay"
	PartlyNight Icon = "partlyNight"
	Cloudy      Icon = "cloudy"
	Fog         Icon = "fog"
	Rain        Icon = "rain"
	Thunder     Icon = "thunder"
	Snow        Icon = "snow"
)

// Describe gives words and an icon for a WMO weather code, as Open-Meteo
// documents the codes.
func Describe(code uint8, isDay bool) (string, Icon) {
	clear, partly := ClearDay, PartlyDay
	if !isDay {
		clear, partly = ClearNight, PartlyNight
	}
	switch code {
	case 0:
		return "Clear", clear
	case 1:
		return "Mostly clear", partly
	case 2:
		return "Partly cloudy", partly
	case 3:
		return "Cloudy", Cloudy
	case 45, 48:
		return "Fog", Fog
	case 51, 53, 55:
		return "Drizzle", Rain
	case 56, 57:
		return "Freezing drizzle", Rain
	case 61:
		return "Light rain", Rain
	case 63:
		return "Rain", Rain
	case 65:
		return "Heavy rain", Rain
	case 66, 67:
		return "Freezing rain", Rain
	case 71:
		return "Light snow", Snow
	case 73, 77:
		return "Snow", Snow
	case 75:
		return "Heavy snow", Snow
	case 80, 81, 82:
		return "Rain showers", Rain
	case 85, 86:
		return "Snow showers", Snow
	case 95:
		return "Thunderstorm", Thunder
	case 96, 99:
		return "Thunderstorm with hail", Thunder
	}
	return "Unknown", Cloudy
}
