package platform

import (
	"testing"
	"time"
)

// Before any change of the computer's zone, the zone read now gives the
// local time that Go read at startup.
func TestTheLocalZoneMatchesGosUntilItChanges(t *testing.T) {
	now := time.Now()
	_, want := now.Zone()
	if _, got := now.In(LocalZone()).Zone(); got != want {
		t.Errorf("offset %d, want %d", got, want)
	}
}
