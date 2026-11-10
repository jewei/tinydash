package platform

import (
	"syscall"
	"time"
)

// LocalZone returns the computer's time zone as it is now. Go reads the
// zone once, at the first use of time.Local, so it would miss a change
// while TinyDash runs. Windows names its zones differently from IANA, so
// this is the current offset from UTC as a fixed zone: right for times
// near now, an hour off for a time on the other side of a daylight saving
// change.
func LocalZone() *time.Location {
	var info syscall.Timezoneinformation
	id, err := syscall.GetTimeZoneInformation(&info)
	if err != nil {
		return time.Local
	}
	bias := info.Bias
	switch id {
	case syscall.TIME_ZONE_ID_STANDARD:
		bias += info.StandardBias
	case syscall.TIME_ZONE_ID_DAYLIGHT:
		bias += info.DaylightBias
	}
	// The bias is minutes west of UTC.
	return time.FixedZone("Local", -int(bias)*60)
}
