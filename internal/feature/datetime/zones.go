package datetime

import (
	"fmt"
	"strings"
	"sync"
	"time"

	"github.com/jewei/tinydash/internal/zoneinfo"
)

// zone is a place whose clock a query names.
type zone struct {
	loc   *time.Location
	label string
}

// aliases are names that people type for zones, beyond IANA city names.
var aliases = [...][2]string{
	{"pacific", "America/Los_Angeles"},
	{"pacific time", "America/Los_Angeles"},
	{"pt", "America/Los_Angeles"},
	{"mountain", "America/Denver"},
	{"mountain time", "America/Denver"},
	{"central", "America/Chicago"},
	{"central time", "America/Chicago"},
	{"eastern", "America/New_York"},
	{"eastern time", "America/New_York"},
	{"et", "America/New_York"},
	{"nyc", "America/New_York"},
	{"sf", "America/Los_Angeles"},
	{"san francisco", "America/Los_Angeles"},
	{"seattle", "America/Los_Angeles"},
	{"la", "America/Los_Angeles"},
	{"boston", "America/New_York"},
	{"washington", "America/New_York"},
	{"uk", "Europe/London"},
	{"england", "Europe/London"},
	{"britain", "Europe/London"},
	{"france", "Europe/Paris"},
	{"germany", "Europe/Berlin"},
	{"spain", "Europe/Madrid"},
	{"italy", "Europe/Rome"},
	{"netherlands", "Europe/Amsterdam"},
	{"india", "Asia/Kolkata"},
	{"mumbai", "Asia/Kolkata"},
	{"delhi", "Asia/Kolkata"},
	{"bangalore", "Asia/Kolkata"},
	{"china", "Asia/Shanghai"},
	{"beijing", "Asia/Shanghai"},
	{"hong kong", "Asia/Hong_Kong"},
	{"taiwan", "Asia/Taipei"},
	{"japan", "Asia/Tokyo"},
	{"korea", "Asia/Seoul"},
	{"malaysia", "Asia/Kuala_Lumpur"},
	{"kl", "Asia/Kuala_Lumpur"},
	{"singapore", "Asia/Singapore"},
	{"thailand", "Asia/Bangkok"},
	{"vietnam", "Asia/Ho_Chi_Minh"},
	{"indonesia", "Asia/Jakarta"},
	{"philippines", "Asia/Manila"},
	{"australia", "Australia/Sydney"},
	{"new zealand", "Pacific/Auckland"},
	{"uae", "Asia/Dubai"},
	{"brazil", "America/Sao_Paulo"},
	{"argentina", "America/Argentina/Buenos_Aires"},
	{"mexico", "America/Mexico_City"},
	{"canada", "America/Toronto"},
	{"south africa", "Africa/Johannesburg"},
	{"utc", "UTC"},
	{"gmt", "Etc/GMT"},
}

// fixedHours are unambiguous abbreviations that name a fixed offset: `pst`
// stays UTC−8 in summer.
var fixedHours = map[string]int{
	"pst": -8, "pdt": -7, "mst": -7, "mdt": -6, "est": -5, "edt": -4,
	"cet": 1, "cest": 2, "jst": 9, "kst": 9, "sgt": 8, "myt": 8, "aest": 10,
}

// cities maps a lowercase city, IANA name, or alias to its IANA name.
var cities = sync.OnceValue(func() map[string]string {
	out := make(map[string]string, 2*len(zoneNames)+len(aliases))
	for _, name := range zoneNames {
		lower := strings.ToLower(name)
		// `Etc/GMT+5` is UTC−5 (POSIX signs), so `gmt+5` must not find it by
		// its last part. The full name still works.
		if !strings.HasPrefix(lower, "etc/") {
			city := strings.ReplaceAll(lastPart(lower), "_", " ")
			if _, ok := out[city]; !ok {
				out[city] = name
			}
		}
		out[lower] = name
	}
	for _, a := range aliases {
		out[a[0]] = a[1]
	}
	return out
})

var locations sync.Map // IANA name -> *time.Location

func location(name string) (*time.Location, bool) {
	if loc, ok := locations.Load(name); ok {
		return loc.(*time.Location), true
	}
	loc, err := zoneinfo.Load(name)
	if err != nil {
		return nil, false
	}
	locations.Store(name, loc)
	return loc, true
}

func lastPart(name string) string {
	return name[strings.LastIndexByte(name, '/')+1:]
}

// localZone is the zone of now, which carries the user's time zone.
func localZone(now time.Time) zone {
	return zone{loc: now.Location(), label: "Local time"}
}

// findZone reads a lowercase place name. local reports `local` or `here`.
func findZone(place string, now time.Time) (z zone, local, ok bool) {
	place = strings.TrimSpace(place)
	for strings.HasSuffix(place, " time") {
		place = strings.TrimSuffix(place, " time")
	}
	place = strings.TrimSpace(place)
	if place == "local" || place == "here" {
		return localZone(now), true, true
	}
	if hours, ok := fixedHours[place]; ok {
		seconds := hours * 3600
		return zone{loc: time.FixedZone("", seconds), label: "UTC" + offsetText(seconds)}, false, true
	}
	name, ok := cities()[place]
	if !ok {
		return zone{}, false, false
	}
	loc, ok := location(name)
	if !ok {
		return zone{}, false, false
	}
	return zone{loc: loc, label: strings.ReplaceAll(lastPart(name), "_", " ")}, false, true
}

func offsetText(seconds int) string {
	sign := '+'
	if seconds < 0 {
		sign = '-'
		seconds = -seconds
	}
	minutes := seconds / 60
	return fmt.Sprintf("%c%02d:%02d", sign, minutes/60, minutes%60)
}

// instants are every instant a wall-clock time names in z: none in a
// daylight-saving gap, two when clocks fall back, earlier first.
func (z zone) instants(wall time.Time) []time.Time {
	var out []time.Time
	// Each offset in force within 14 hours of the wall time shows at one of
	// these probes, unless clocks change twice in four days.
	for _, probe := range [...]time.Duration{-48 * time.Hour, 0, 48 * time.Hour} {
		_, offset := wall.Add(probe).In(z.loc).Zone()
		t := wall.Add(-time.Duration(offset) * time.Second).In(z.loc)
		if sameWall(t, wall) && !containsInstant(out, t) {
			out = append(out, t)
		}
	}
	if len(out) == 2 && out[1].Before(out[0]) {
		out[0], out[1] = out[1], out[0]
	}
	return out
}

func sameWall(t, wall time.Time) bool {
	y, m, d := t.Date()
	wy, wm, wd := wall.Date()
	return y == wy && m == wm && d == wd && t.Hour() == wall.Hour() && t.Minute() == wall.Minute() && t.Second() == wall.Second()
}

func containsInstant(ts []time.Time, t time.Time) bool {
	for _, u := range ts {
		if u.Equal(t) {
			return true
		}
	}
	return false
}

// CityClock is a city's clock in the widget pane. The launcher keeps the
// time current from the offset and reads it again each time the pane loads,
// so a clock change shows by the next open.
type CityClock struct {
	// Name is the place as the user typed it.
	Name string
	// OffsetSeconds is seconds east of UTC, now.
	OffsetSeconds int
	// Difference is how far it is from local time: `+8h`, `−5h 30m`, or
	// `Same time`.
	Difference string
}

// MaxClockCities is the most cities the clocks widget shows next to local
// time.
const MaxClockCities = 3

// City is the clock of a place the user named, or false when no time
// zone has that name. Local time is not a city: the widget always shows it.
func City(name string, now time.Time) (CityClock, bool) {
	z, local, ok := findZone(strings.ToLower(name), now)
	if !ok || local {
		return CityClock{}, false
	}
	_, there := now.In(z.loc).Zone()
	_, here := now.Zone()
	return CityClock{Name: strings.TrimSpace(name), OffsetSeconds: there, Difference: difference(there - here)}, true
}

func difference(seconds int) string {
	if seconds == 0 {
		return "Same time"
	}
	sign := "+"
	if seconds < 0 {
		sign = "−"
		seconds = -seconds
	}
	minutes := seconds / 60
	switch h, m := minutes/60, minutes%60; {
	case h == 0:
		return fmt.Sprintf("%s%dm", sign, m)
	case m == 0:
		return fmt.Sprintf("%s%dh", sign, h)
	default:
		return fmt.Sprintf("%s%dh %dm", sign, h, m)
	}
}
