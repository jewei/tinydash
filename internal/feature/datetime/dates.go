package datetime

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

// Dates are time.Time values at midnight UTC. The Rust app's dates ran from
// year -262143 to 262142; a step past them gives no answer there, so it
// gives none here.
const (
	minYear = -262143
	maxYear = 262142
)

func civil(t time.Time) time.Time {
	return time.Date(t.Year(), t.Month(), t.Day(), 0, 0, 0, 0, time.UTC)
}

// makeDate is the date, or false when the month has no such day.
func makeDate(year, month, day int) (time.Time, bool) {
	if year < minYear || year > maxYear || month < 1 || month > 12 || day < 1 || day > daysIn(year, month) {
		return time.Time{}, false
	}
	return time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC), true
}

func daysIn(year, month int) int {
	return time.Date(year, time.Month(month)+1, 0, 0, 0, 0, 0, time.UTC).Day()
}

func inRange(date time.Time) bool {
	return date.Year() >= minYear && date.Year() <= maxYear
}

func dayNumber(date time.Time) int64 { return date.Unix() / 86400 }

// addMonths keeps the day, or takes the month's last day when it is
// shorter: 31 January + 1 month is 28 February.
func addMonths(date time.Time, months int64) (time.Time, bool) {
	total := int64(date.Year())*12 + int64(date.Month()) - 1 + months
	year, month := total/12, total%12
	if month < 0 {
		year, month = year-1, month+12
	}
	if year < minYear || year > maxYear {
		return time.Time{}, false
	}
	day := min(date.Day(), daysIn(int(year), int(month)+1))
	return makeDate(int(year), int(month)+1, day)
}

func yearText(year int) string {
	if year < 0 || year > 9999 {
		return fmt.Sprintf("%+05d", year)
	}
	return fmt.Sprintf("%04d", year)
}

// longDate is `Friday, 25 December 2026`.
func longDate(date time.Time) string {
	return date.Format("Monday, 2 January ") + yearText(date.Year())
}

// shortDate is `Fri 25 Dec 2026`.
func shortDate(date time.Time) string {
	return date.Format("Mon 2 Jan ") + yearText(date.Year())
}

func isoText(date time.Time) string {
	return yearText(date.Year()) + date.Format("-01-02")
}

// isoDate reads a 4-digit year and a 1- or 2-digit month and day, such as
// `2026-10-17` or `2026-1-5`. `12-3-4` is a subtraction, not a date.
func isoDate(word string) (time.Time, bool) {
	parts := strings.Split(word, "-")
	if len(parts) != 3 || !digits(parts[0], 4, 4) || !digits(parts[1], 1, 2) || !digits(parts[2], 1, 2) {
		return time.Time{}, false
	}
	year, _ := strconv.Atoi(parts[0])
	month, _ := strconv.Atoi(parts[1])
	day, _ := strconv.Atoi(parts[2])
	return makeDate(year, month, day)
}

func digits(s string, minLen, maxLen int) bool {
	return len(s) >= minLen && len(s) <= maxLen && allDigits(s)
}

// allDigits is true for "", as Rust's `bytes().all` was.
func allDigits(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return false
		}
	}
	return true
}

// dateMath reads `base (+|-) n unit ...`, or a phrase that relativePhrase
// reads. A bare date also answers, with its weekday.
func dateMath(query string, today time.Time) (core.Result, bool) {
	if phrase, ok := relativePhrase(query); ok {
		query = phrase
	}
	var words []string
	for _, word := range strings.Fields(strings.ReplaceAll(query, "+", " + ")) {
		words = append(words, splitMinus(word)...)
	}
	end := len(words)
	for i, word := range words {
		if word == "+" || word == "-" {
			end = i
			break
		}
	}
	base, ok := namedDay(words[:end], today)
	if !ok {
		return core.Result{}, false
	}
	date := base.date
	steps := words[end:]
	for len(steps) > 0 {
		sign := 1
		switch steps[0] {
		case "+":
		case "-":
			sign = -1
		default:
			return core.Result{}, false
		}
		if date, steps, ok = shift(date, sign, steps[1:]); !ok {
			return core.Result{}, false
		}
	}
	from := ""
	if base.label != "" {
		from = " · " + base.label + " is " + shortDate(base.date)
	}
	var relative string
	switch days := dayNumber(date) - dayNumber(today); {
	case days == 0:
		relative = "today"
	case days == 1:
		relative = "tomorrow"
	case days == -1:
		relative = "yesterday"
	case days > 0:
		relative = fmt.Sprintf("in %d days", days)
	default:
		relative = fmt.Sprintf("%d days ago", -days)
	}
	result := answer(longDate(date), relative+from, isoText(date))
	result.Actions[0].Label = "Copy Date"
	return result, true
}

// relativePhrase rewrites `in 10 days`, `3 days from now`, `a week ago`,
// `2 weeks from tomorrow`, `2 months 3 days after dec 25`, or `3 days before
// next friday` in the `base (+|-) n unit ...` form.
func relativePhrase(query string) (string, bool) {
	if amount, ok := strings.CutPrefix(query, "in "); ok {
		// `in 2 weeks + 1 day` already has its signs.
		if steps, ok := splitSteps(amount); ok {
			amount = strings.Join(steps, " + ")
		}
		return "today + " + amount, true
	}
	amount, sign, base := "", "", ""
	if a, ok := strings.CutSuffix(query, " ago"); ok {
		amount, sign, base = a, "-", "today"
	} else {
		found := false
		for _, w := range [...][2]string{{" after ", "+"}, {" from ", "+"}, {" before ", "-"}} {
			if a, b, ok := strings.Cut(query, w[0]); ok {
				amount, sign, base, found = a, w[1], b, true
				if base == "now" {
					base = "today"
				}
				break
			}
		}
		if !found {
			return "", false
		}
	}
	steps, ok := splitSteps(amount)
	if !ok {
		return "", false
	}
	for i, step := range steps {
		steps[i] = sign + " " + step
	}
	return base + " " + strings.Join(steps, " "), true
}

// splitSteps reads `2 months 3 days`, `2 months and 3 days`, `a week`, or
// `3d` as steps such as `2 months`, for shift to check.
func splitSteps(amount string) ([]string, bool) {
	var words []string
	for _, word := range strings.Fields(amount) {
		if word = strings.TrimRight(word, ","); word != "and" {
			words = append(words, word)
		}
	}
	var steps []string
	for i := 0; i < len(words); i++ {
		number := words[i]
		if number == "a" || number == "an" {
			number = "1"
		}
		switch {
		case allDigits(number):
			if i+1 == len(words) {
				return nil, false
			}
			i++
			steps = append(steps, number+" "+words[i])
		case number[0] >= '0' && number[0] <= '9':
			steps = append(steps, number)
		default:
			return nil, false
		}
	}
	return steps, len(steps) > 0
}

// splitMinus splits a minus sign from the words it touches, as in `-3`,
// `today-3d`, or `friday-1w`. The query cannot put spaces around every `-`,
// as it does for `+`, because ISO dates contain `-`; a word that starts with
// a digit keeps its `-`.
func splitMinus(word string) []string {
	before, after, ok := strings.Cut(word, "-")
	if !ok || after == "" || !asciiLetters(before) {
		return []string{word}
	}
	if before == "" {
		return []string{"-", after}
	}
	return []string{before, "-", after}
}

func asciiLetters(s string) bool {
	for i := 0; i < len(s); i++ {
		if c := s[i] | 0x20; c < 'a' || c > 'z' {
			return false
		}
	}
	return true
}

// shift applies one `<n> <unit>` step, also written together as in `3d` or
// `2weeks`, and returns the words after it.
func shift(date time.Time, sign int, words []string) (time.Time, []string, bool) {
	if len(words) == 0 {
		return time.Time{}, nil, false
	}
	word := words[0]
	words = words[1:]
	split := strings.IndexFunc(word, func(r rune) bool { return r < '0' || r > '9' })
	amountText, unit := word, ""
	if split >= 0 {
		amountText, unit = word[:split], word[split:]
	} else {
		if len(words) == 0 {
			return time.Time{}, nil, false
		}
		unit, words = words[0], words[1:]
	}
	amount, err := strconv.ParseUint(amountText, 10, 32)
	if err != nil {
		return time.Time{}, nil, false
	}
	n := int64(amount) * int64(sign)
	var ok bool
	switch strings.TrimRight(unit, "s") {
	case "day", "d":
		date, ok = addDays(date, n)
	case "week", "w":
		date, ok = addDays(date, 7*n)
	case "month", "mo":
		date, ok = addMonths(date, n)
	case "year", "y":
		// Rust counted the months in 32 bits.
		if 12*amount > 1<<32-1 {
			return time.Time{}, nil, false
		}
		date, ok = addMonths(date, 12*n)
	default:
		return time.Time{}, nil, false
	}
	return date, words, ok
}

func addDays(date time.Time, days int64) (time.Time, bool) {
	// Past ±200 million days the result is out of range anyway; stopping
	// here keeps the arithmetic below from overflowing.
	if days > 2e8 || days < -2e8 {
		return time.Time{}, false
	}
	date = date.AddDate(0, 0, int(days))
	return date, inRange(date)
}

// dayCount reads `days until dec 25`, `days till next friday`, `days since
// 2026-01-01`, or `days between 1 mar and 30 may`.
func dayCount(query string, today time.Time) (core.Result, bool) {
	rest, ok := strings.CutPrefix(query, "days ")
	if !ok {
		return core.Result{}, false
	}
	day := func(text string) (namedDate, bool) { return namedDay(strings.Fields(text), today) }
	var from, to time.Time
	var subtitle string
	if rest, ok := strings.CutPrefix(rest, "between "); ok {
		firstText, secondText, ok := strings.Cut(rest, " and ")
		if !ok {
			return core.Result{}, false
		}
		first, ok := day(firstText)
		if !ok {
			return core.Result{}, false
		}
		second, ok := day(secondText)
		if !ok {
			return core.Result{}, false
		}
		to = second.date
		// Without years, `days between dec 20 and jan 5` counts to the
		// January after.
		if !first.hasYear && !second.hasYear && second.date.Before(first.date) {
			month, dom := int(second.date.Month()), second.date.Day()
			if to, ok = inYear(month, dom, first.date.Year(), 1); !ok {
				return core.Result{}, false
			}
			if to.Before(first.date) {
				if to, ok = inYear(month, dom, first.date.Year()+1, 1); !ok {
					return core.Result{}, false
				}
			}
		}
		from = first.date
		subtitle = "from " + shortDate(from) + " to " + shortDate(to)
	} else {
		word, target, ok := strings.Cut(rest, " ")
		if !ok {
			return core.Result{}, false
		}
		var ahead bool
		switch word {
		case "until", "till":
			ahead = true
		case "since":
		default:
			return core.Result{}, false
		}
		named, ok := day(target)
		if !ok {
			return core.Result{}, false
		}
		// Without a year, `days until jan 1` in October means the coming
		// one, and `days since dec 25` in October the last one.
		month, dom := int(named.date.Month()), named.date.Day()
		to = named.date
		switch {
		case !named.hasYear && ahead && named.date.Before(today):
			to, ok = inYear(month, dom, named.date.Year()+1, 1)
		case !named.hasYear && !ahead && named.date.After(today):
			to, ok = inYear(month, dom, named.date.Year()-1, -1)
		}
		if !ok {
			return core.Result{}, false
		}
		from = today
		direction := "since"
		if !to.Before(today) {
			direction = "until"
		}
		subtitle = direction + " " + longDate(to)
	}
	days := dayNumber(to) - dayNumber(from)
	if days < 0 {
		days = -days
	}
	weeks := ""
	switch {
	case days/7 == 0:
	case days%7 == 0:
		weeks = plural(days/7, "week") + " · "
	default:
		weeks = plural(days/7, "week") + " " + plural(days%7, "day") + " · "
	}
	result := answer(plural(days, "day"), weeks+subtitle, strconv.FormatInt(days, 10))
	result.Actions[0].Label = "Copy Days"
	return result, true
}

func plural(count int64, unit string) string {
	if count == 1 {
		return "1 " + unit
	}
	return fmt.Sprintf("%d %ss", count, unit)
}

// namedDate is a day the user named.
type namedDate struct {
	date time.Time
	// label is the words of a day relative to today, such as `next friday`.
	label   string
	hasYear bool
}

// namedDay reads `today`, `tomorrow`, `yesterday`, `next friday`, `last
// monday`, an ISO date, or a date with a month name, such as `25 dec` or
// `December 25, 2026`.
func namedDay(words []string, today time.Time) (namedDate, bool) {
	relative := func(date time.Time) (namedDate, bool) {
		return namedDate{date: date, label: strings.Join(words, " "), hasYear: true}, true
	}
	switch len(words) {
	case 1:
		switch words[0] {
		case "today":
			return relative(today)
		case "tomorrow":
			return relative(today.AddDate(0, 0, 1))
		case "yesterday":
			return relative(today.AddDate(0, 0, -1))
		}
		date, ok := isoDate(words[0])
		return namedDate{date: date, hasYear: true}, ok
	case 2:
		if words[0] == "next" || words[0] == "last" {
			weekday, ok := parseWeekday(words[1])
			if !ok {
				return namedDate{}, false
			}
			if words[0] == "next" {
				ahead := (7 + int(weekday) - int(today.Weekday())) % 7
				if ahead == 0 {
					ahead = 7
				}
				return relative(today.AddDate(0, 0, ahead))
			}
			behind := (7 + int(today.Weekday()) - int(weekday)) % 7
			if behind == 0 {
				behind = 7
			}
			return relative(today.AddDate(0, 0, -behind))
		}
	}
	return monthDate(words, today)
}

// weekdayNames are the long names, whose first three letters are the short
// ones.
var weekdayNames = [...]string{"sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"}

// parseWeekday reads a weekday's name or its first three letters. The query
// is lowercase already.
func parseWeekday(word string) (time.Weekday, bool) {
	for i, name := range weekdayNames {
		if word == name || word == name[:3] {
			return time.Weekday(i), true
		}
	}
	return 0, false
}

var monthNames = [...]string{
	"january", "february", "march", "april", "may", "june",
	"july", "august", "september", "october", "november", "december",
}

// monthDate reads `25 dec`, `dec 25`, `25th december 2026`, or `December
// 25, 2026`. Without a year, the date is in this year, or for `feb 29`, the
// next leap year.
func monthDate(words []string, today time.Time) (namedDate, bool) {
	if len(words) != 2 && len(words) != 3 {
		return namedDate{}, false
	}
	day, okDay := dayOfMonth(words[0])
	month, okMonth := monthOf(words[1])
	if !okDay || !okMonth {
		if day, okDay = dayOfMonth(words[1]); !okDay {
			return namedDate{}, false
		}
		if month, okMonth = monthOf(words[0]); !okMonth {
			return namedDate{}, false
		}
	}
	if len(words) == 2 {
		date, ok := inYear(month, day, today.Year(), 1)
		return namedDate{date: date}, ok
	}
	if len(words[2]) != 4 {
		return namedDate{}, false
	}
	// Rust's i32 parse also took a sign, as in `+202`.
	year, err := strconv.ParseInt(words[2], 10, 32)
	if err != nil {
		return namedDate{}, false
	}
	date, ok := makeDate(int(year), month, day)
	return namedDate{date: date, hasYear: true}, ok
}

// inYear is the day in year, or in the first year after it (step 1) or
// before it (step -1) that has the day: only leap years have 29 February.
func inYear(month, day, year, step int) (time.Time, bool) {
	for n := range 8 {
		if date, ok := makeDate(year+n*step, month, day); ok {
			return date, true
		}
	}
	return time.Time{}, false
}

// dayOfMonth reads `25`, `25th`, or `25,`.
func dayOfMonth(word string) (int, bool) {
	word = strings.TrimRight(word, ",")
	for _, suffix := range [...]string{"st", "nd", "rd", "th"} {
		if digits, ok := strings.CutSuffix(word, suffix); ok {
			word = digits
			break
		}
	}
	if !digits(word, 1, 2) {
		return 0, false
	}
	day, _ := strconv.Atoi(word)
	return day, true
}

// monthOf reads a month by its name or a start of 3 letters or more: `dec`,
// `sept`.
func monthOf(word string) (int, bool) {
	word = strings.TrimRight(word, ".,")
	if len(word) < 3 {
		return 0, false
	}
	for i, name := range monthNames {
		if strings.HasPrefix(name, word) {
			return i + 1, true
		}
	}
	return 0, false
}
