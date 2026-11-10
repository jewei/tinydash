package unixtime

import (
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

var now = time.Date(2026, 10, 7, 12, 0, 0, 0, time.UTC)

func copied(t *testing.T, r core.Result) []string {
	t.Helper()
	var out []string
	for _, a := range r.Actions {
		c, ok := a.Action.(core.Copy)
		if !ok {
			t.Fatalf("unexpected action %#v", a.Action)
		}
		out = append(out, a.Label+": "+c.Text)
	}
	return out
}

func TestReadsUnixTimesInSecondsAndMilliseconds(t *testing.T) {
	for _, tc := range []struct {
		text  string
		now   time.Time
		title string
		sub   string
		iso   string
	}{
		{"1791354301", now, "Wed 7 Oct 2026, 06:25:01", "Wed 7 Oct 2026, 06:25:01 UTC · 6 hours ago", "2026-10-07T06:25:01Z"},
		{" 1791354301250 ", now.In(time.FixedZone("UTC+8", 8*3600)), "Wed 7 Oct 2026, 14:25:01.250", "Wed 7 Oct 2026, 06:25:01.250 UTC · 6 hours ago", "2026-10-07T06:25:01.250Z"},
	} {
		r, ok := Answer(tc.text, tc.now)
		if !ok {
			t.Errorf("%q is a Unix time", tc.text)
			continue
		}
		if r.Kind != core.KindDateTime || r.Title != tc.title || r.Subtitle != tc.sub {
			t.Errorf("%q: %s %q / %q", tc.text, r.Kind, r.Title, r.Subtitle)
		}
		want := []string{"Copy Local Time: " + tc.title, "Copy UTC Time: " + r.Subtitle[:len(r.Subtitle)-len(" · 6 hours ago")], "Copy ISO 8601: " + tc.iso}
		got := copied(t, r)
		for i := range want {
			if got[i] != want[i] {
				t.Errorf("%q action %d: %q, want %q", tc.text, i, got[i], want[i])
			}
		}
	}
	for _, text := range []string{"999999999", "12345678901", "4300000000", "17913543x1", "+791354301", "755", ""} {
		if _, ok := Answer(text, now); ok {
			t.Errorf("%q is not a Unix time", text)
		}
	}
}

func TestDecodesTheDigitsAndTheirUnit(t *testing.T) {
	got, ok := Decode(" 1791354301\n", now)
	want := Time{
		Raw: "1791354301", Local: "Wed 7 Oct 2026, 06:25:01", UTC: "Wed 7 Oct 2026, 06:25:01 UTC",
		ISO: "2026-10-07T06:25:01Z", Relative: "6 hours ago",
	}
	if !ok || got != want {
		t.Errorf("seconds: %+v", got)
	}
	if got, ok := Decode("1791354301250", now); !ok || !got.Milliseconds || got.Raw != "1791354301250" {
		t.Errorf("milliseconds: %+v", got)
	}
}

func TestRelative(t *testing.T) {
	for seconds, want := range map[int64]string{
		30:                   "Just now",
		2 * 3600:             "In 2 hours",
		-86_400:              "1 day ago",
		3_590:                "In 1 hour",
		-(23*3_600 + 40*60):  "1 day ago",
		29*86_400 + 13*3_600: "In 1 month",
		-355 * 86_400:        "1 year ago",
		59 * 60:              "In 59 minutes",
	} {
		if got := relative(seconds); got != want {
			t.Errorf("relative(%d) = %q, want %q", seconds, got, want)
		}
	}
}
