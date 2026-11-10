//go:build !windows

package platform

import (
	"os"
	"time"
)

// LocalZone returns the computer's time zone as it is now. Go reads the
// zone once, at the first use of time.Local, so it would miss a change
// while TinyDash runs; this reads /etc/localtime, which macOS and Linux
// point at the zone the user chose, at each call. With TZ set, which no
// change of the system's zone touches, or without /etc/localtime, it is
// time.Local.
func LocalZone() *time.Location {
	if _, set := os.LookupEnv("TZ"); set {
		return time.Local
	}
	data, err := os.ReadFile("/etc/localtime")
	if err != nil {
		return time.Local
	}
	zone, err := time.LoadLocationFromTZData("Local", data)
	if err != nil {
		return time.Local
	}
	return zone
}
