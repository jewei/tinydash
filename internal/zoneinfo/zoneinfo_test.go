package zoneinfo

import (
	"testing"
	"time"
)

func TestLoadsZonesWithTheirRules(t *testing.T) {
	for _, c := range []struct {
		name   string
		at     time.Time
		offset int
	}{
		{"Asia/Kuala_Lumpur", time.Date(2026, 11, 9, 0, 0, 0, 0, time.UTC), 8 * 3600},
		{"America/New_York", time.Date(2026, 1, 15, 0, 0, 0, 0, time.UTC), -5 * 3600},
		{"America/New_York", time.Date(2026, 7, 15, 0, 0, 0, 0, time.UTC), -4 * 3600},
		{"US/Pacific", time.Date(2026, 7, 15, 0, 0, 0, 0, time.UTC), -7 * 3600},
		{"UTC", time.Date(2026, 7, 15, 0, 0, 0, 0, time.UTC), 0},
	} {
		loc, err := Load(c.name)
		if err != nil {
			t.Errorf("%s: %v", c.name, err)
			continue
		}
		if loc.String() != c.name {
			t.Errorf("%s: named %q", c.name, loc.String())
		}
		if _, offset := c.at.In(loc).Zone(); offset != c.offset {
			t.Errorf("%s at %s: offset %d, want %d", c.name, c.at, offset, c.offset)
		}
	}
}

func TestRejectsUnknownZones(t *testing.T) {
	for _, name := range []string{"", "Local", "Mars/Olympus_Mons", "../etc/passwd"} {
		if _, err := Load(name); err == nil {
			t.Errorf("%q loaded", name)
		}
	}
}
