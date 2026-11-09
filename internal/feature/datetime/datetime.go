// Package datetime answers date and time queries offline.
//
//   - Current time: `now`, `time in tokyo`, `london time`
//   - Conversion: `10am pacific`, `9:30 tokyo to london`, `tomorrow 3pm in kl`
//   - Dates: `today + 3 days`, `next friday + 2 weeks`, `in 10 days`,
//     `2028-02-28 + 1 week`, `3 days from now`, `a week ago`, `2 weeks after dec 25`
//   - Day counts: `days until dec 25`, `days since 2026-01-01`,
//     `days between 1 mar and 30 may`
//   - City clocks for the widget pane: `Tokyo`, `kl`, `Europe/London`
package datetime

import (
	"strconv"
	"strings"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

// Answers reads a query. now carries the user's time zone.
func Answers(query string, now time.Time) []core.Result {
	query = strings.ToLower(strings.TrimSpace(query))
	if place, ok := currentTimePlace(query); ok {
		if answer, ok := currentTime(place, now); ok {
			return []core.Result{answer}
		}
	}
	if answers, ok := conversion(query, now); ok {
		return answers
	}
	today := civil(now)
	if answer, ok := dayCount(query, today); ok {
		return []core.Result{answer}
	}
	if answer, ok := dateMath(query, today); ok {
		return []core.Result{answer}
	}
	return nil
}

// IsDateOrClock reports whether the whole query is a full `YYYY-MM-DD` date
// or a clock time, such as `2026-10-17` or `3pm`. The calculator leaves these
// alone: it would read them as a subtraction or a length. Shorter dates, such
// as `2026-1-5`, stay subtractions.
func IsDateOrClock(query string) bool {
	query = strings.ToLower(strings.TrimSpace(query))
	if _, ok := isoDate(query); ok && len(query) == 10 {
		return true
	}
	words := strings.Fields(query)
	_, _, used, ok := clock(words)
	return ok && used == len(words)
}

// currentTimePlace reads `time in X`, `now in X`, `X time`, or plain
// `now` or `time`.
func currentTimePlace(query string) (string, bool) {
	if query == "now" || query == "time" {
		return "local", true
	}
	for _, prefix := range [...]string{"time in ", "now in ", "time at "} {
		if place, ok := strings.CutPrefix(query, prefix); ok {
			return place, true
		}
	}
	return strings.CutSuffix(query, " time")
}

func currentTime(place string, now time.Time) (core.Result, bool) {
	z, _, ok := findZone(place, now)
	if !ok {
		return core.Result{}, false
	}
	there := now.In(z.loc)
	_, offset := there.Zone()
	return answer(
		there.Format("15:04 · Monday, 2 January"),
		z.label+" (UTC"+offsetText(offset)+")",
		there.Format("15:04"),
	), true
}

// conversion reads `[day] <clock> [source] [to|in target]`. At least one
// place is needed. A clock in a daylight-saving gap gives no answers, and
// one that clocks repeat gives two.
func conversion(query string, now time.Time) ([]core.Result, bool) {
	words := strings.Fields(query)
	var day dayWord
	hasDay := false
	if len(words) > 0 {
		if day, hasDay = readDayWord(words[0]); hasDay {
			words = words[1:]
		}
	}
	hour, minute, used, ok := clock(words)
	if !ok {
		return nil, false
	}
	rest := words[used:]
	sourceName, targetName := strings.Join(rest, " "), ""
	for i, w := range rest {
		if w == "to" || w == "in" {
			sourceName, targetName = strings.Join(rest[:i], " "), strings.Join(rest[i+1:], " ")
			break
		}
	}
	if sourceName == "" && targetName == "" {
		return nil, false
	}
	source, target := localZone(now), localZone(now)
	if sourceName != "" {
		if source, _, ok = findZone(sourceName, now); !ok {
			return nil, false
		}
	}
	if targetName != "" {
		if target, _, ok = findZone(targetName, now); !ok {
			return nil, false
		}
	}
	date := day.date
	if !hasDay || !day.isDate {
		date = civil(now.In(source.loc)).AddDate(0, 0, day.offset)
	}
	wall := time.Date(date.Year(), date.Month(), date.Day(), hour, minute, 0, 0, time.UTC)
	answers := []core.Result{}
	for _, instant := range source.instants(wall) {
		there := instant.In(target.loc)
		_, offset := instant.Zone()
		answers = append(answers, answer(
			there.Format("15:04 · Mon 2 Jan"),
			instant.Format("15:04 Mon 2 Jan ")+source.label+" (UTC"+offsetText(offset)+") in "+target.label,
			there.Format("15:04"),
		))
	}
	return answers, true
}

// dayWord is a date, or a number of days from today.
type dayWord struct {
	date   time.Time
	isDate bool
	offset int
}

func readDayWord(word string) (dayWord, bool) {
	switch word {
	case "today":
		return dayWord{}, true
	case "tomorrow":
		return dayWord{offset: 1}, true
	case "yesterday":
		return dayWord{offset: -1}, true
	}
	date, ok := isoDate(word)
	return dayWord{date: date, isDate: true}, ok
}

// clock reads `10am`, `10 am`, `10:30`, `10:30 p.m.`, `22:15`, `noon`, or
// `midnight` from the start of words, and says how many words it used.
func clock(words []string) (hour, minute, used int, ok bool) {
	if len(words) == 0 {
		return 0, 0, 0, false
	}
	first := words[0]
	switch first {
	case "noon":
		return 12, 0, 1, true
	case "midnight":
		return 0, 0, 1, true
	}
	end := strings.IndexFunc(first, func(r rune) bool { return (r < '0' || r > '9') && r != ':' })
	if end < 0 {
		end = len(first)
	}
	number, suffix := first[:end], first[end:]
	const (
		none = iota
		am
		pm
	)
	meridiem := func(word string) int {
		switch strings.ReplaceAll(word, ".", "") {
		case "am":
			return am
		case "pm":
			return pm
		}
		return none
	}
	half, used := none, 1
	if suffix != "" {
		if half = meridiem(suffix); half == none {
			return 0, 0, 0, false
		}
	} else if len(words) > 1 {
		if half = meridiem(words[1]); half != none {
			used = 2
		}
	}
	hourText, minuteText, hasColon := strings.Cut(number, ":")
	switch {
	case hasColon && len(minuteText) == 2:
		// 8 bits hold every valid minute and hour, and an int holds 8
		// bits on every platform, so the conversion cannot wrap.
		m, err := strconv.ParseUint(minuteText, 10, 8)
		if err != nil {
			return 0, 0, 0, false
		}
		minute = int(m)
	case !hasColon && half != none:
	default:
		return 0, 0, 0, false
	}
	h, err := strconv.ParseUint(hourText, 10, 8)
	if err != nil {
		return 0, 0, 0, false
	}
	hour = int(h)
	if half != none {
		if hour < 1 || hour > 12 {
			return 0, 0, 0, false
		}
		hour %= 12
		if half == pm {
			hour += 12
		}
	}
	if hour > 23 || minute > 59 {
		return 0, 0, 0, false
	}
	return hour, minute, used, true
}

func answer(title, subtitle, copy string) core.Result {
	return core.Result{
		ID:       core.ID("time:" + title),
		Kind:     core.KindDateTime,
		Title:    title,
		Subtitle: subtitle,
		Icon:     core.SymbolIcon(core.SymbolClock),
		Actions:  []core.ResultAction{core.Do("Copy Time", core.Copy{Text: copy})},
	}
}
