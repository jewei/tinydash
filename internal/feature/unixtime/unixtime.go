// Package unixtime answers a Unix time typed in seconds or milliseconds with
// the local, UTC, and ISO 8601 times.
package unixtime

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

// Seconds from 2001-09-09 to 2103-02-05: shorter or longer numbers are more
// likely counts or IDs than times.
const (
	minSeconds = 1_000_000_000
	maxSeconds = 4_200_000_000
)

// Answer decodes 10 digits as seconds or 13 as milliseconds since 1970. The
// local time is in now's location.
func Answer(query string, now time.Time) (core.Result, bool) {
	text := strings.TrimSpace(query)
	if strings.Trim(text, "0123456789") != "" || len(text) != 10 && len(text) != 13 {
		return core.Result{}, false
	}
	number, err := strconv.ParseInt(text, 10, 64)
	if err != nil {
		return core.Result{}, false
	}
	milliseconds := len(text) == 13
	seconds := number
	if milliseconds {
		seconds = number / 1000
	}
	if seconds < minSeconds || seconds >= maxSeconds {
		return core.Result{}, false
	}
	layout, iso := "Mon 2 Jan 2006, 15:04:05", time.RFC3339
	at := time.Unix(seconds, 0)
	if milliseconds {
		layout, iso = layout+".000", "2006-01-02T15:04:05.000Z07:00"
		at = time.UnixMilli(number)
	}
	utc := at.UTC()
	localText := at.In(now.Location()).Format(layout)
	utcText := utc.Format(layout) + " UTC"
	isoText := utc.Format(iso)
	return core.Result{
		ID:       core.ID("unix:" + text),
		Kind:     core.KindDateTime,
		Title:    localText,
		Subtitle: utcText + " · " + relative(seconds-now.Unix()),
		Icon:     core.SymbolIcon(core.SymbolClock),
		Actions: []core.ResultAction{
			core.Do("Copy Local Time", core.Copy{Text: localText}),
			core.Do("Copy UTC Time", core.Copy{Text: utcText}),
			core.Do("Copy ISO 8601", core.Copy{Text: isoText}),
		},
	}, true
}

// relative says `3 days ago` or `In 2 hours`, in the largest whole unit.
func relative(seconds int64) string {
	distance := uint64(seconds)
	if seconds < 0 {
		distance = uint64(-seconds)
	}
	if distance < 45 {
		return "Just now"
	}
	// Each unit with how many of it make the next one, so that rounding up
	// to 60 minutes shows 1 hour instead.
	units := []struct {
		size uint64
		name string
		next uint64
	}{
		{60, "minute", 60},
		{3_600, "hour", 24},
		{86_400, "day", 30},
		{30 * 86_400, "month", 12},
		{365 * 86_400, "year", ^uint64(0)},
	}
	index := 0
	for i, u := range units {
		if distance >= u.size {
			index = i
		}
	}
	round := func(size uint64) uint64 { return (distance + size/2) / size }
	for round(units[index].size) >= units[index].next {
		index++
	}
	count := round(units[index].size)
	amount := fmt.Sprintf("%d %s", count, units[index].name)
	if count != 1 {
		amount += "s"
	}
	if seconds < 0 {
		return amount + " ago"
	}
	return "In " + amount
}
