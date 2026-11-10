package datetime

import (
	"strings"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/zoneinfo"
)

// testNow is Sunday, 4 October 2026, 12:00 UTC, seen from Kuala Lumpur.
func testNow(t *testing.T) time.Time {
	t.Helper()
	loc, err := zoneinfo.Load("Asia/Kuala_Lumpur")
	if err != nil {
		t.Fatal(err)
	}
	return time.Date(2026, 10, 4, 12, 0, 0, 0, time.UTC).In(loc)
}

func mustDate(t *testing.T, text string) time.Time {
	t.Helper()
	date, ok := isoDate(text)
	if !ok {
		t.Fatalf("isoDate(%q) failed", text)
	}
	return date
}

func copied(r core.Result) string { return r.Actions[0].Action.(core.Copy).Text }

// math is the date that dateMath copies for a query on 2026-10-03, or "".
func math(t *testing.T, query string) string {
	t.Helper()
	r, ok := dateMath(query, mustDate(t, "2026-10-03"))
	if !ok {
		return ""
	}
	return copied(r)
}

func TestDateArithmetic(t *testing.T) {
	for _, c := range []struct{ query, want string }{
		{"today + 3 days", "2026-10-06"},
		{"in 2 weeks", "2026-10-17"},
		{"next friday + 2 weeks", "2026-10-23"},
		{"next saturday", "2026-10-10"},
		{"last monday", "2026-09-28"},
		{"2028-02-28 + 1 day", "2028-02-29"},
		{"2026-01-31 + 1 month", "2026-02-28"},
		{"2026-10-03 - 1 year", "2025-10-03"},
		{"tomorrow+1d", "2026-10-05"},
		{"2026-10-17", "2026-10-17"},
		{"2026-1-5 + 1 week", "2026-01-12"},
		{"0001-01-01 - 2 years", "-0001-01-01"},
		{"9999-12-31 + 1 day", "+10000-01-01"},
		{"9999-12-31 + 300000 years", ""},
		{"2026-10-03 + 4294967295 days", ""},
		{"2026-10-03 + 357913942 years", ""},
	} {
		if got := math(t, c.query); got != c.want {
			t.Errorf("%q: got %q, want %q", c.query, got, c.want)
		}
	}
}

func TestMinusSignMayTouchItsNumber(t *testing.T) {
	for _, c := range []struct{ glued, spaced string }{
		{"today -3 days", "today - 3 days"},
		{"today-3d", "today - 3d"},
		{"next friday -1 week", "next friday - 1 week"},
		{"next friday-1w", "next friday - 1w"},
		{"2026-10-17 -2d", "2026-10-17 - 2d"},
	} {
		spaced := math(t, c.spaced)
		if spaced == "" {
			t.Errorf("%q: no answer", c.spaced)
		}
		if got := math(t, c.glued); got != spaced {
			t.Errorf("%q: got %q, want %q", c.glued, got, spaced)
		}
	}
}

func TestDateMathSubtitleNamesTheBaseDate(t *testing.T) {
	for _, c := range []struct{ query, want string }{
		{"tomorrow + 1 day", "in 2 days · tomorrow is Sun 4 Oct 2026"},
		{"yesterday - 1 day", "2 days ago · yesterday is Fri 2 Oct 2026"},
		{"next friday + 2 weeks", "in 20 days · next friday is Fri 9 Oct 2026"},
		{"last monday", "5 days ago · last monday is Mon 28 Sep 2026"},
		{"in 2 weeks", "in 14 days · today is Sat 3 Oct 2026"},
		{"2026-10-03 - 1 year", "365 days ago"},
	} {
		r, ok := dateMath(c.query, mustDate(t, "2026-10-03"))
		if !ok || r.Subtitle != c.want {
			t.Errorf("%q: got %q, want %q", c.query, r.Subtitle, c.want)
		}
	}
}

func TestReadsRelativePhrases(t *testing.T) {
	for _, c := range []struct{ query, want string }{
		{"3 days from now", "2026-10-06"},
		{"3d from today", "2026-10-06"},
		{"3 days ago", "2026-09-30"},
		{"a week ago", "2026-09-26"},
		{"2 weeks from tomorrow", "2026-10-18"},
		{"3 days after next friday", "2026-10-12"},
		{"2 months 3 days after 2026-06-05", "2026-08-08"},
		{"2 months and 3 days before 2026-06-05", "2026-04-02"},
		{"28 days before mar 12", "2026-02-12"},
		{"in 2 weeks + 1 day", "2026-10-18"},
		{"in 2 weeks and 1 day", "2026-10-18"},
		{"3 days", ""},
		{"days ago", ""},
		{"3 apples ago", ""},
		{"a week from monday", ""},
		{"3 days after", ""},
	} {
		if got := math(t, c.query); got != c.want {
			t.Errorf("%q: got %q, want %q", c.query, got, c.want)
		}
	}
}

func TestReadsDatesWithMonthNames(t *testing.T) {
	for _, c := range []struct{ query, want string }{
		{"dec 25", "2026-12-25"},
		{"25 dec", "2026-12-25"},
		{"25th december 2027", "2027-12-25"},
		{"december 25, 2026 + 1 week", "2027-01-01"},
		{"sept 1", "2026-09-01"},
		{"feb 29", "2028-02-29"},
		{"jan 2026", ""},
		{"may", ""},
		{"ma 5", ""},
		{"dec 32", ""},
		{"feb 30", ""},
		{"feb 29 2026", ""},
		{"dec 25 26", ""},
		{"june 5 notes", ""},
	} {
		if got := math(t, c.query); got != c.want {
			t.Errorf("%q: got %q, want %q", c.query, got, c.want)
		}
	}
}

func TestCountsDays(t *testing.T) {
	count := func(query string, today time.Time) (title, subtitle string, ok bool) {
		r, ok := dayCount(query, today)
		return r.Title, r.Subtitle, ok
	}
	oct3, mar2028 := mustDate(t, "2026-10-03"), mustDate(t, "2028-03-01")
	for _, c := range []struct {
		query           string
		today           time.Time
		title, subtitle string
	}{
		{"days until dec 25", oct3, "83 days", "11 weeks 6 days · until Friday, 25 December 2026"},
		{"days until jan 1", oct3, "90 days", ""},
		{"days till next friday", oct3, "6 days", ""},
		{"days since jan 1", oct3, "275 days", ""},
		{"days since dec 25", oct3, "282 days", ""},
		{"days until 2026-10-01", oct3, "2 days", "since Thursday, 1 October 2026"},
		{"days until tomorrow", oct3, "1 day", "until Sunday, 4 October 2026"},
		{"days between 3 march and 30 may", oct3, "88 days", "12 weeks 4 days · from Tue 3 Mar 2026 to Sat 30 May 2026"},
		{"days between 2026-01-15 and 2026-01-01", oct3, "14 days", ""},
		// Without years, the second date is the first one after the first.
		{"days between dec 20 and jan 5", oct3, "16 days", "2 weeks 2 days · from Sun 20 Dec 2026 to Tue 5 Jan 2027"},
		{"days between jan 5 and dec 20", oct3, "349 days", ""},
		{"days between dec 20 and 2026-01-05", oct3, "349 days", ""},
		{"days between feb 29 and mar 1", oct3, "1 day", ""},
		{"days until feb 29", oct3, "514 days", ""},
		{"days since feb 29", oct3, "947 days", ""},
		{"days until feb 29", mar2028, "1460 days", ""},
		{"days since feb 29", mar2028, "1 day", ""},
		{"days until 2026-10-17", oct3, "14 days", "2 weeks · until Saturday, 17 October 2026"},
	} {
		title, subtitle, ok := count(c.query, c.today)
		if !ok || title != c.title || (c.subtitle != "" && subtitle != c.subtitle) {
			t.Errorf("%q: got %q, %q, want %q, %q", c.query, title, subtitle, c.title, c.subtitle)
		}
	}
	for _, query := range []string{"days", "days until", "days until soon", "days between today", "days ago"} {
		if _, _, ok := count(query, oct3); ok {
			t.Errorf("%q: want no answer", query)
		}
	}
	if got := Answers("Days until dec 25", testNow(t))[0].Actions[0].Label; got != "Copy Days" {
		t.Errorf("label: got %q", got)
	}
}

func TestOrdinaryWordsAreNotDates(t *testing.T) {
	for _, query := range []string{"in", "today plus", "next", "next week", "2026-13-01", "12-3-4", "in bed"} {
		if got := math(t, query); got != "" {
			t.Errorf("%q: got %q", query, got)
		}
	}
}

func TestParsesClocks(t *testing.T) {
	for _, c := range []struct {
		words              []string
		hour, minute, used int
		ok                 bool
	}{
		{[]string{"10am"}, 10, 0, 1, true},
		{[]string{"10", "p.m."}, 22, 0, 2, true},
		{[]string{"12am"}, 0, 0, 1, true},
		{[]string{"9:30"}, 9, 30, 1, true},
		{[]string{"noon"}, 12, 0, 1, true},
		{[]string{"13pm"}, 0, 0, 0, false},
		{[]string{"10"}, 0, 0, 0, false},
		{[]string{"24:00"}, 0, 0, 0, false},
		{[]string{"9:5"}, 0, 0, 0, false},
		// An hour past 32 bits must not wrap to a small or negative int.
		{[]string{"4294967295:00"}, 0, 0, 0, false},
		{[]string{"4294967307pm"}, 0, 0, 0, false},
		{[]string{"99999999999:00"}, 0, 0, 0, false},
	} {
		hour, minute, used, ok := clock(c.words)
		if hour != c.hour || minute != c.minute || used != c.used || ok != c.ok {
			t.Errorf("%q: got %d:%d used %d %v", c.words, hour, minute, used, ok)
		}
	}
}

func convert(t *testing.T, query string) []core.Result {
	t.Helper()
	results, ok := conversion(query, testNow(t))
	if !ok {
		t.Fatalf("%q: not a conversion", query)
	}
	return results
}

func TestConvertsBetweenZonesWithDaylightSaving(t *testing.T) {
	for _, c := range []struct{ query, title string }{
		// 2026-07-01 10:00 in Los Angeles (UTC−7) is 02:00 next day in Tokyo.
		{"2026-07-01 10am pacific to tokyo", "02:00 · Thu 2 Jul"},
		// PST is a fixed offset even in summer.
		{"2026-07-01 10am pst to utc", "18:00 · Wed 1 Jul"},
		{"2026-7-1 10am pst to utc", "18:00 · Wed 1 Jul"},
		{"2026-07-01 10am pacific time to utc", "17:00 · Wed 1 Jul"},
	} {
		results := convert(t, c.query)
		if len(results) != 1 || results[0].Title != c.title {
			t.Errorf("%q: got %v", c.query, results)
		}
	}
	if got := len(Answers("10am pacific time", testNow(t))); got != 1 {
		t.Errorf("10am pacific time: %d answers", got)
	}
}

func TestClockChangesGiveNoOrTwoAnswers(t *testing.T) {
	// US clocks skip 02:00–03:00 on 2026-03-08 and repeat 01:00 on 2026-11-01.
	if got := convert(t, "2026-03-08 2:30am new york to utc"); len(got) != 0 {
		t.Errorf("gap: got %v", got)
	}
	got := convert(t, "2026-11-01 1:30am new york to utc")
	if len(got) != 2 || !strings.HasPrefix(got[0].Title, "05:30") || !strings.HasPrefix(got[1].Title, "06:30") {
		t.Errorf("repeat: got %v", got)
	}
}

func TestPosixEtcZonesAreNotFoundByOffset(t *testing.T) {
	now := testNow(t)
	if _, _, ok := findZone("gmt+5", now); ok {
		t.Error("gmt+5 found a zone")
	}
	if _, _, ok := findZone("etc/gmt+5", now); !ok {
		t.Error("etc/gmt+5 found no zone")
	}
}

func TestCityNamesSharedByZonesFindTheFirst(t *testing.T) {
	z, _, ok := findZone("west", testNow(t))
	if !ok || z.loc.String() != "Australia/West" {
		t.Errorf("west: got %v", z.loc)
	}
}

func TestEveryZoneLoads(t *testing.T) {
	for _, name := range zoneNames {
		if _, ok := location(name); !ok {
			t.Errorf("%s does not load", name)
		}
	}
}

func TestCityClocksKnowTheirOffsetAndDifference(t *testing.T) {
	now := testNow(t)
	// On 4 October 2026, Tokyo is UTC+9 and New York is UTC−4.
	tokyo, ok := City(" Tokyo ", now)
	if !ok || tokyo != (CityClock{Name: "Tokyo", OffsetSeconds: 9 * 3600, Difference: "+1h"}) {
		t.Errorf("Tokyo: got %+v", tokyo)
	}
	for _, c := range []struct {
		name   string
		offset int
	}{{"kl", 8 * 3600}, {"pst", -8 * 3600}, {"America/New_York", -4 * 3600}} {
		if got, ok := City(c.name, now); !ok || got.OffsetSeconds != c.offset {
			t.Errorf("%s: got %+v", c.name, got)
		}
	}
	for _, unknown := range []string{"local", "Atlantis", ""} {
		if got, ok := City(unknown, now); ok {
			t.Errorf("%q: got %+v", unknown, got)
		}
	}
	for _, c := range []struct {
		seconds int
		want    string
	}{{0, "Same time"}, {8 * 3600, "+8h"}, {-(5*3600 + 30*60), "−5h 30m"}, {45 * 60, "+45m"}} {
		if got := difference(c.seconds); got != c.want {
			t.Errorf("difference(%d): got %q, want %q", c.seconds, got, c.want)
		}
	}
}

func TestCurrentTimeNeedsATimeWord(t *testing.T) {
	now := testNow(t)
	for _, query := range []string{"time in tokyo", "london time", "now"} {
		if len(Answers(query, now)) == 0 {
			t.Errorf("%q: no answer", query)
		}
	}
	for _, query := range []string{"tokyo", "time machine", "10am", "meeting at 10am"} {
		if got := Answers(query, now); len(got) != 0 {
			t.Errorf("%q: got %v", query, got)
		}
	}
}
