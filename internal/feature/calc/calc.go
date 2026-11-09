// Package calc is the calculator: arithmetic, unit conversion, and currency
// conversion, as instant answers, and the history of copied calculations.
// Phrases such as `$30 + 20%` or `10 plus 5` are rewritten first.
package calc

import (
	"fmt"
	"math"
	"strconv"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
)

// Rates is the exchange rate table, as currency.Rates gives it. A nil Rates
// means that currency rates are off.
type Rates interface {
	// PerEuro is how many units of the currency one euro buys.
	PerEuro(code string) (float64, bool)
	// Currencies counts the currencies of the table, the euro included.
	Currencies() int
	// PublishedOn is the date of the table, YYYY-MM-DD.
	PublishedOn() string
}

const (
	// MaxCalculations is the most calculations kept in history.
	MaxCalculations = 50
	// MaxExpression is the longest expression, in characters, kept in
	// history.
	MaxExpression = 500
	// historyRows is the most history rows one search shows.
	historyRows = 20
)

var calculatorIcon = core.SymbolIcon(core.SymbolCalculator)

// Answer is the calculator's answer to query, if it has one.
func Answer(query string, rates Rates) (core.Result, bool) {
	value, usedRates, missing, ok := evaluateQuery(query, rates)
	if !ok {
		if missing != "" && rates != nil {
			return noRateAnswer(query, missing, rates), true
		}
		return core.Result{}, false
	}
	subtitle := "= " + query
	if usedRates && rates != nil {
		subtitle = fmt.Sprintf("%s · ECB rates from %s", query, rates.PublishedOn())
	}
	return core.Result{
		ID:       core.ID("calc:" + query),
		Kind:     core.KindCalculation,
		Title:    value,
		Subtitle: subtitle,
		Icon:     calculatorIcon,
		Actions:  []core.ResultAction{core.Do("Copy Result", core.CopyCalculation{Query: query})},
	}, true
}

// Result is the result of query as the calculator answers it, for copying.
func Result(query string, rates Rates) (string, bool) {
	value, _, _, ok := evaluateQuery(query, rates)
	return value, ok
}

// Expression is the expression in query: a spreadsheet user types `=5+3`
// for `5+3`. A query that asks for the history stays as it is.
func Expression(query string) string {
	if _, ok := historyFilter(query); ok {
		return query
	}
	return strings.TrimPrefix(query, "=")
}

// historyFilter is the words after `=` alone, or after `=` and a space,
// which ask for the history.
func historyFilter(query string) (string, bool) {
	rest, ok := strings.CutPrefix(strings.TrimSpace(query), "=")
	if !ok {
		return "", false
	}
	if rest == "" {
		return "", true
	}
	r, _ := utf8.DecodeRuneInString(rest)
	return rest, unicode.IsSpace(r)
}

// Calculation is a calculation the user copied.
type Calculation struct {
	ID         int64
	Expression string
	Result     string
}

// History is the copied calculations, newest first.
type History struct {
	entries []Calculation
}

// NewHistory holds entries, newest first.
func NewHistory(entries []Calculation) History { return History{entries: entries} }

// Get returns the calculation with id.
func (h History) Get(id int64) (Calculation, bool) {
	for _, entry := range h.entries {
		if entry.ID == id {
			return entry, true
		}
	}
	return Calculation{}, false
}

// Answer lists the history for `=`, and the calculations whose expression or
// result holds every word for `= words`. It is false for any other query,
// such as `=5+3`, which a spreadsheet user means as a calculation.
func (h History) Answer(query string) ([]core.Result, bool) {
	filter, ok := historyFilter(query)
	if !ok {
		return nil, false
	}
	words := strings.Fields(strings.ToLower(filter))
	results := []core.Result{}
	for _, entry := range h.entries {
		if len(results) == historyRows {
			break
		}
		text := strings.ToLower(entry.Expression + " " + entry.Result)
		if containsAll(text, words) {
			results = append(results, historyResult(entry))
		}
	}
	return results, true
}

func containsAll(text string, words []string) bool {
	for _, word := range words {
		if !strings.Contains(text, word) {
			return false
		}
	}
	return true
}

func historyResult(entry Calculation) core.Result {
	return core.Result{
		ID:       core.ID("calc-history:" + strconv.FormatInt(entry.ID, 10)),
		Kind:     core.KindCalculation,
		Title:    entry.Result,
		Subtitle: "= " + entry.Expression,
		Icon:     calculatorIcon,
		// The saved result, as copied then; rates may have changed since.
		Actions: []core.ResultAction{
			core.Do("Copy Result", core.CopySavedCalculation{ID: entry.ID}),
			core.Do("Copy Expression", core.Copy{Text: entry.Expression}),
			core.Do("Delete from History", core.DeleteCalculation{ID: entry.ID}),
		},
	}
}

// noRateAnswer says that the rates lack a currency that a query needs, such
// as `100 twd to myr`, instead of giving no answer.
func noRateAnswer(query, code string, rates Rates) core.Result {
	return core.Result{
		ID:    core.ID("calc:" + query),
		Kind:  core.KindCalculation,
		Title: "No exchange rate for " + code,
		Subtitle: fmt.Sprintf("The ECB rates from %s have %d currencies, and %s is not one of them.",
			rates.PublishedOn(), rates.Currencies(), code),
		Icon: calculatorIcon,
	}
}

// RatesOffAnswer says, while currency rates are off, that a query such as
// `100 usd to eur` needs them, and offers to turn them on.
func RatesOffAnswer(query string) (core.Result, bool) {
	if !needsRates(query) {
		return core.Result{}, false
	}
	return core.Result{
		ID:       core.ID("calc:" + query),
		Kind:     core.KindCalculation,
		Title:    "Currency rates are off",
		Subtitle: "Turn them on to convert. TinyDash then downloads the daily ECB rate table.",
		Icon:     calculatorIcon,
		Actions:  []core.ResultAction{core.Do("Turn On Currency Rates", core.TurnOnCurrencyRates{})},
	}, true
}

// needsRates reports whether the query asks for an exchange rate, found
// with a stand-in rate of 1.
func needsRates(query string) bool {
	if !strings.ContainsFunc(query, isDigit) {
		return false
	}
	in, ok := input(query)
	if !ok {
		return false
	}
	probe := &rateSource{probe: true}
	if n, err := parse(in); err == nil {
		_, _ = evaluate(n, probe)
	}
	return probe.asked
}

// evaluateQuery returns the answer text and whether it needed exchange
// rates, or, when there is no answer, the first currency that the rates
// lack.
func evaluateQuery(query string, rates Rates) (value string, usedRates bool, missing string, ok bool) {
	if !strings.ContainsFunc(query, isDigit) && query != "pi" && query != "π" {
		return "", false, "", false
	}
	if isDateOrClock(query) {
		return "", false, "", false
	}
	in, ok := input(query)
	if !ok {
		return "", false, "", false
	}
	n, err := parse(in)
	if err != nil {
		return "", false, "", false
	}
	source := &rateSource{rates: rates}
	q, err := evaluate(n, source)
	if err != nil {
		return "", false, source.missing, false
	}
	value = strings.TrimSpace(q.String())
	// A bare number echoes itself, also as the evaluator writes it after
	// the rewrites (`30 percent` as `30%`); that is not an answer.
	echo := bare(value)
	if value == "" || echo == bare(query) || echo == bare(in) || roundedTooLong(query, value) {
		return "", false, "", false
	}
	if !source.asked {
		return value, false, "", true
	}
	if rounded, ok := money(value); ok {
		value = rounded
	}
	return value, true, "", true
}

func bare(text string) string { return strings.Join(strings.Fields(text), "") }

// roundedTooLong keeps the Rust app's answers for `floor`, `ceil`, and
// `round`: it gave none with a run of 14 or more digits, because fend
// computed those through an f64 and got them wrong. The goldens pin that.
func roundedTooLong(query, value string) bool {
	query = asciiLower(query)
	if !strings.Contains(query, "floor") && !strings.Contains(query, "ceil") && !strings.Contains(query, "round") {
		return false
	}
	runs := strings.FieldsFunc(value, func(r rune) bool {
		return !(isDigit(r) || 'a' <= r && r <= 'z' || 'A' <= r && r <= 'Z')
	})
	for _, run := range runs {
		if len(run) >= 14 && strings.ContainsFunc(run, isDigit) {
			return true
		}
	}
	return false
}

// money writes `approx. 401.7094 MYR` as `401.71 MYR`: the subtitle already
// says that the value comes from daily rates. An amount below 1 keeps 3
// significant digits, so `1 IDR to EUR` is not `0.00 EUR`. A fraction, such
// as `401 83/117 MYR`, stays as it is. A unit with a `/`, such as
// `MYR / kg`, is rounded.
func money(value string) (string, bool) {
	amount, currency, ok := strings.Cut(strings.TrimPrefix(value, "approx. "), " ")
	if !ok || currency != "" && isDigit(rune(currency[0])) {
		return "", false
	}
	if !isFloat(amount) {
		return "", false
	}
	a, _ := strconv.ParseFloat(amount, 64)
	places := 2
	if a != 0 && math.Abs(a) < 1 {
		// The exponent of the shortest decimal is floor(log10(|a|)), also
		// where log10 of a power of ten rounds below it.
		e := strconv.FormatFloat(math.Abs(a), 'e', -1, 64)
		exponent, _ := strconv.Atoi(e[strings.IndexByte(e, 'e')+1:])
		places = 2 - exponent
	}
	return strconv.FormatFloat(a, 'f', places, 64) + " " + currency, true
}

// isDateOrClock reports whether the whole query is a full YYYY-MM-DD date or
// a clock time, such as `2026-10-17` or `3pm`. The date answers take these:
// the calculator would read them as a subtraction or a length. Shorter
// dates, such as `2026-1-5`, stay subtractions. The datetime feature has
// the same check; features do not import each other.
func isDateOrClock(query string) bool {
	query = strings.ToLower(strings.TrimSpace(query))
	if len(query) == 10 {
		if _, err := time.Parse(time.DateOnly, query); err == nil {
			return true
		}
	}
	words := strings.Fields(query)
	used, ok := clock(words)
	return ok && used == len(words)
}

// clock reads a clock time at the start of words, such as `3pm`, `3 pm`,
// `10:30`, or `noon`, and returns how many words it used.
func clock(words []string) (int, bool) {
	if len(words) == 0 {
		return 0, false
	}
	first := words[0]
	if first == "noon" || first == "midnight" {
		return 1, true
	}
	meridiem := func(word string) bool {
		word = strings.ReplaceAll(word, ".", "")
		return word == "am" || word == "pm"
	}
	end := strings.IndexFunc(first, func(r rune) bool { return !isDigit(r) && r != ':' })
	if end < 0 {
		end = len(first)
	}
	number, suffix := first[:end], first[end:]
	hasMeridiem, used := false, 1
	switch {
	case suffix != "" && !meridiem(suffix):
		return 0, false
	case suffix != "":
		hasMeridiem = true
	case len(words) > 1 && meridiem(words[1]):
		hasMeridiem, used = true, 2
	}
	hour, minute := 0, 0
	if h, m, ok := strings.Cut(number, ":"); ok {
		if len(m) != 2 {
			return 0, false
		}
		var errH, errM error
		hour, errH = strconv.Atoi(h)
		minute, errM = strconv.Atoi(m)
		if errH != nil || errM != nil {
			return 0, false
		}
	} else {
		if !hasMeridiem {
			return 0, false
		}
		var err error
		if hour, err = strconv.Atoi(number); err != nil {
			return 0, false
		}
	}
	if hasMeridiem && (hour < 1 || hour > 12) {
		return 0, false
	}
	if hasMeridiem {
		hour %= 12
	}
	return used, hour >= 0 && hour < 24 && minute >= 0 && minute < 60
}
