package calc

import (
	"reflect"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

type testRates struct {
	date    string
	perEuro map[string]float64
}

func (r testRates) PerEuro(code string) (float64, bool) {
	if code == "EUR" {
		return 1, true
	}
	rate, ok := r.perEuro[code]
	return rate, ok
}
func (r testRates) Currencies() int     { return len(r.perEuro) + 1 }
func (r testRates) PublishedOn() string { return r.date }

// fixture is the Rust tests' rate table.
var fixture = testRates{"2026-10-02", map[string]float64{"USD": 1.17, "MYR": 4.7, "CNY": 8.0}}

func value(query string) string {
	v, _ := Result(query, nil)
	return v
}

func valueWith(rates Rates, query string) string {
	v, _ := Result(query, rates)
	return v
}

func expect(t *testing.T, get func(string) string, cases map[string]string) {
	t.Helper()
	for query, want := range cases {
		if got := get(query); got != want {
			t.Errorf("%q = %q, want %q", query, got, want)
		}
	}
}

func expectNone(t *testing.T, get func(string) string, queries ...string) {
	t.Helper()
	for _, query := range queries {
		if got := get(query); got != "" {
			t.Errorf("%q = %q, want no answer", query, got)
		}
	}
}

func TestHistoryListsAndFiltersCopiedCalculations(t *testing.T) {
	history := NewHistory([]Calculation{{2, "5 ft to cm", "152.4 cm"}, {1, "12 * 8", "96"}})
	titles := func(query string) []string {
		results, ok := history.Answer(query)
		if !ok {
			return nil
		}
		titles := []string{}
		for _, r := range results {
			titles = append(titles, r.Title)
		}
		return titles
	}
	for query, want := range map[string][]string{
		"=":         {"152.4 cm", "96"},
		"= CM":      {"152.4 cm"},
		"= 12 8":    {"96"},
		"= nothing": {},
		"12 * 8":    nil,
		"=12*8":     nil,
	} {
		if got := titles(query); !reflect.DeepEqual(got, want) {
			t.Errorf("%q lists %q, want %q", query, got, want)
		}
	}
	rows, _ := history.Answer("= 96")
	var labels []string
	for _, a := range rows[0].Actions {
		labels = append(labels, a.Label)
	}
	if want := []string{"Copy Result", "Copy Expression", "Delete from History"}; !reflect.DeepEqual(labels, want) {
		t.Errorf("labels %q, want %q", labels, want)
	}
}

func TestHistoryShowsAtMost20Rows(t *testing.T) {
	var entries []Calculation
	for i := range MaxCalculations {
		entries = append(entries, Calculation{int64(i), "1 + 1", "2"})
	}
	if rows, _ := NewHistory(entries).Answer("="); len(rows) != historyRows {
		t.Errorf("%d rows, want %d", len(rows), historyRows)
	}
}

func TestCopyingAnAnswerEvaluatesItAgain(t *testing.T) {
	answer, _ := Answer("12 * 8", nil)
	if answer.Actions[0].Action != (core.CopyCalculation{Query: "12 * 8"}) {
		t.Errorf("action %#v", answer.Actions[0].Action)
	}
	expect(t, value, map[string]string{"12 * 8": "96"})
	expectNone(t, value, "hello")
}

func TestALeadingEqualsSignIsACalculation(t *testing.T) {
	for query, want := range map[string]string{"=12*8": "12*8", "12*8": "12*8", "= 12*8": "= 12*8", "=": "="} {
		if got := Expression(query); got != want {
			t.Errorf("Expression(%q) = %q, want %q", query, got, want)
		}
	}
	answer, _ := Answer(Expression("=12*8"), nil)
	if answer.Actions[0].Action != (core.CopyCalculation{Query: "12*8"}) {
		t.Errorf("action %#v", answer.Actions[0].Action)
	}
}

func TestAHistoryRowCopiesItsSavedResult(t *testing.T) {
	history := NewHistory([]Calculation{{7, "12 * 8", "96"}})
	rows, _ := history.Answer("=")
	if rows[0].Actions[0].Action != (core.CopySavedCalculation{ID: 7}) {
		t.Errorf("action %#v", rows[0].Actions[0].Action)
	}
	if entry, ok := history.Get(7); !ok || entry.Result != "96" {
		t.Errorf("Get(7) = %v, %v", entry, ok)
	}
	if _, ok := history.Get(8); ok {
		t.Error("Get(8) found an entry")
	}
}

func TestEvaluatesArithmeticAndUnits(t *testing.T) {
	expect(t, value, map[string]string{
		"12 * 8":                    "96",
		"sqrt(144)":                 "12",
		"8 Mbps to MBps":            "1 MBps",
		"5 ft to cm":                "152.4 cm",
		"1 gb to mb":                "1000000000000 mb",
		"3 kg to lb":                "approx. 6.6138678655 lbs",
		"90 min to hours":           "1.5 hours",
		"0.1 + 0.2":                 "0.3",
		"1/3":                       "approx. 0.3333333333",
		"pi * 2":                    "approx. 6.2831853072",
		"pi":                        "approx. 3.1415926536",
		"2^64":                      "18446744073709551616",
		"10 % 3":                    "1",
		"100 celsius to fahrenheit": "212 fahrenheit",
		"212 °F to °C":              "100 °C",
		// fend writes `per` as `/`, which does not repeat the query.
		"5 m per s":           "5 m / s",
		"100 km per hour":     "100 km / hour",
		"5 meters per second": "5 meters / second",
	})
	expectNone(t, value, "10 / 0", "12 x 8", "100 c to f", "sinh(40)")
}

func TestIgnoresTextBareNumbersAndErrors(t *testing.T) {
	expectNone(t, value, "safari", "42", "30 percent", "20percent", "−5", "2kg", "1password", "12 +")
}

func TestLeavesDatesAndClockTimesToTheDateAnswers(t *testing.T) {
	expectNone(t, value, "2026-10-17", "3pm", "10am", "10:30 PM", "3 pm", "noon 5")
	expect(t, value, map[string]string{
		"2026 - 10": "2016",
		"2026-10":   "2016",
		"12-3-4":    "5",
		"100-5-3":   "92",
		"2026-1-5":  "2020",
		// Not a date: February has no 30th.
		"2026-02-30": "1994",
	})
}

func TestOffersToTurnOnRatesOnlyForCurrencyQueries(t *testing.T) {
	answer, ok := RatesOffAnswer("100 usd to eur")
	if !ok || answer.Actions[0].Action != (core.TurnOnCurrencyRates{}) {
		t.Errorf("%v %#v", ok, answer)
	}
	for _, query := range []string{"100 USD MYR", "20k rmb to myr", "1 btc to usd", "5 €€"} {
		if _, ok := RatesOffAnswer(query); !ok {
			t.Errorf("%q: no rates-off answer", query)
		}
	}
	for _, query := range []string{"12 * 8", "5 ft to cm", "usd", "rmb", "100 usd to qqq", "-$5 +"} {
		if _, ok := RatesOffAnswer(query); ok {
			t.Errorf("%q: a rates-off answer", query)
		}
	}
}

func TestConvertsCurrencyWithCachedRates(t *testing.T) {
	value, used, _, _ := evaluateQuery("100 USD MYR", fixture)
	if !used || value != "401.71 MYR" {
		t.Errorf("100 USD MYR = %q, used rates: %v", value, used)
	}
	answer, _ := Answer("100 USD to MYR", fixture)
	if !strings.Contains(answer.Subtitle, "2026-10-02") {
		t.Errorf("subtitle %q", answer.Subtitle)
	}
	expectNone(t, func(q string) string { return valueWith(nil, q) }, "100 USD to MYR")
}

func TestReadsTheNamesPeopleUseForACurrency(t *testing.T) {
	get := func(q string) string { return valueWith(fixture, q) }
	expect(t, get, map[string]string{
		"20k rmb to myr":      "11750.00 MYR",
		"20k RMB to MYR":      "11750.00 MYR",
		"20000 yuan to myr":   "11750.00 MYR",
		"20k renminbi to myr": "11750.00 MYR",
		"100 usd to rmb":      "683.76 CNY",
		"10 dollars to eur":   "8.55 EUR",
	})
	// The ECB table has no Taiwan dollar.
	expectNone(t, get, "100 twd to myr")
}

func TestSaysWhenTheRatesLackACurrency(t *testing.T) {
	row, _ := Answer("100 twd to myr", fixture)
	if row.Title != "No exchange rate for TWD" ||
		row.Subtitle != "The ECB rates from 2026-10-02 have 4 currencies, and TWD is not one of them." ||
		len(row.Actions) != 0 {
		t.Errorf("%#v", row)
	}
	if row, _ := Answer("100 usd to twd", fixture); row.Title != "No exchange rate for TWD" {
		t.Errorf("title %q", row.Title)
	}
	// A query that cannot be read, or that needs no rate, is not this.
	if _, ok := Answer("100 usd to qqq", fixture); ok {
		t.Error("100 usd to qqq answers")
	}
	if row, _ := Answer("100 usd to myr", fixture); row.Title != "401.71 MYR" {
		t.Errorf("title %q", row.Title)
	}
	// Without rates, the rates-off answer speaks instead.
	if _, ok := Answer("100 twd to myr", nil); ok {
		t.Error("100 twd to myr answers without rates")
	}
}

func TestReadsPercentagePhrases(t *testing.T) {
	expect(t, value, map[string]string{
		"30 + 20%":              "36",
		"200 - 10%":             "180",
		"5 kg + 10%":            "5.5 kg",
		"10% of 200":            "20",
		"10% on 200":            "220",
		"20 is what % of 200":   "10%",
		"20 as a % of 200":      "10%",
		"180 is what % off 200": "10%",
		"180 is what % on 150":  "20%",
		"50 to 75 is what %":    "50%",
		"40 to 90 as %":         "125%",
		"3/20 is what %":        "15%",
		"0.35 as %":             "35%",
		// `to` and `in` read as `as`.
		"0.35 to %":           "35%",
		"1/8 in %":            "12.5%",
		"0.35 to percent":     "35%",
		"50 to 75 in %":       "50%",
		"20 is 10% of what":   "200",
		"220 is 10% on what":  "200",
		"180 is 10% off what": "200",
		"2e-3%":               "0.002%",
	})
	// Not a char boundary where a suffix would start.
	expectNone(t, value, "10% of what", "2 Äpfel", "5 €€")
	// Any amount of money needs rates.
	expect(t, func(q string) string { return valueWith(fixture, q) }, map[string]string{
		"$30 + 20%":    "$36",
		"30% of $200":  "$60",
		"15% OFF $200": "$170",
	})
	if _, ok := RatesOffAnswer("$30 + 20%"); !ok {
		t.Error("$30 + 20% needs rates")
	}
}

func TestTakesPercentagesInTurn(t *testing.T) {
	expect(t, value, map[string]string{
		"200 - 10% - 5%":            "171",
		"100 + 10% + 20%":           "132",
		"30 plus 20 percent":        "36",
		"30 + 20Percent":            "36",
		"20 is what percent of 200": "10%",
		// A remainder, not a percentage.
		"5 + 10 % 3": "6",
	})
}

func TestGivesNoAnswerForAPercentageInAnotherShape(t *testing.T) {
	expectNone(t, value,
		// It would add a hundredth: 100 + 0.1 + 5.
		"100 + 10% + 5", "(100 + 10%) * 2", "round(200 - 10%)", "100 - (10%)", "100 - 2 * 5%",
		"5% + 100 + 10%", "50 * 2 + 10%", "100 * 10% + 20%", "5 + 10% to fraction", "2 dozen 10%",
		// The percentage is not a plain number.
		"5 - 1e-2%", "10 - -5%",
		// `200 / 10%` would be (200 / 10)%, which is 20%.
		"200 / 10%", "200 * 10%", "10% + 20%", "(10% + 20%) * 2", "5 * -10%",
		// A part of the question is not a plain amount.
		"50 * 2 is what % of 400", "1 + 2 is what %",
		// `percent` that is not a word of its own.
		"100 - (20 percent)", "5+20percent",
	)
	if _, ok := RatesOffAnswer("$30 + 20% + 5"); ok {
		t.Error("$30 + 20% + 5 needs rates")
	}
}

func TestAPercentageOfIsOneValue(t *testing.T) {
	expect(t, value, map[string]string{
		"10% of 200 + 50":             "70",
		"15% of 80 + 80":              "92",
		"10% off 200 + 5":             "185",
		"10% on 200 * 2":              "440",
		"10% of 200 / 4":              "5",
		"5 + 10% of 200":              "25",
		"100 - 10% off 200":           "-80",
		"3 * 10% of 200":              "60",
		"(10% of 200) / 4":            "5",
		"10 percent of 200":           "20",
		"10% of 200 + 20% of 300":     "80",
		"10% of 200 * 10% of 50":      "100",
		"10% off 200 - 5% off 100":    "85",
		"5 + 10% of 200 + 10% of 300": "55",
	})
	expectNone(t, value,
		// After `/` or a power, fend takes the percentage of all that comes
		// before it: 1/2% of 200 is 1.
		"1/2% of 200", "400 / 10% of 200", "400 / 10% off 200", "400 / -10% off 200",
		"2 ^ 10% on 200", "2 ^ -10% of 200", "2 ** 10% of 200", "10% of 200 / 10% of 50",
		"10% of 200 ^ 10% of 50",
		// After `per` or another operator word, the sign belongs to the
		// percentage.
		"400 per -10% off 200", "400 per +10% off 200", "400 mod +10% off 200", "5 choose +10% of 20",
		"1e1mod +10% off 200", "0x10mod +10% off 200", "1e1per -10% off 200", "5²mod +10% off 200",
		"5! + 10% of 200",
		// A sign or a value right before the percentage.
		"10% of 200 - -10% of 50", "-10% off 200", "2 10% of 200", "1e-2% of 200",
		// More than a plain amount after it.
		"10% of 2^3", "10% of 5!", "10% of 200 mod 3", "10% of 200 per 4", "10% of 20% of 200",
		"10% of 200 - 5%",
	)
	expect(t, func(q string) string { return valueWith(fixture, q) }, map[string]string{
		"20% off $50 + $5": "$45",
		"30% of -$200":     "$-60",
		"10% on  $50":      "$55",
	})
}

func TestAPhraseAfterASignNeedsAnAmountOrAParenthesisBeforeIt(t *testing.T) {
	for _, text := range []string{"", "(", "400 -", "5 kg *", "(2) +", "-$5 +", "1,000.5 -", "2^5 +", "10% of 200 +", "5 min -"} {
		if !startsValue(text) {
			t.Errorf("%q does not start a value", text)
		}
	}
	for _, word := range []string{"to", "as", "in", "per", "of", "mod", "and", "AND", "or", "OR", "xor", "XOR", "choose", "nCr", "permute", "nPr"} {
		if startsValue("400 "+word+" -") || startsValue("400"+word+"+") {
			t.Errorf("%q starts a value", word)
		}
	}
	for _, text := range []string{"1e1mod +", "1e1 +", "0x10mod +", "16#ff -", "5²mod +", "5² +", "5kg -", "x2mod -", "pi +", "5! +", "-", "2 * -", "2 ** ", "5 kg to g +"} {
		if startsValue(text) {
			t.Errorf("%q starts a value", text)
		}
	}
}

func TestReadsOtherSpellingsOfSigns(t *testing.T) {
	if got := asciiSigns("1−2×3✕4÷5∕6≠7λ"); got != "1-2 *3 *4/5/6!=7\\" {
		t.Errorf("asciiSigns = %q", got)
	}
	expect(t, value, map[string]string{
		"5 × 3":      "15",
		"5 ×* 3":     "125",
		"12 ÷ 4":     "3",
		"100 − 10%":  "90",
		"1e−2 * 100": "1",
	})
	expectNone(t, value, "5 ×× 3", "5 *× 3", "100 − 10% + 5", "100 ÷ 10% off 200", "2 ×* 10% on 200")
}

func TestRefusesInputThatBindsAName(t *testing.T) {
	for _, query := range []string{
		"(\\x.x + 1) 5", "(λx.x + 1) 5", "(x: x + 1) 5", "(x => x * 2) 5", "f = x: x * 2; f 5",
		"a = 5; a * 2", "(x: x x)(x: x x) 1", "(x => x x)(x => x x) 1", "f = x: f x; f 1",
		"cos = cis; cis 1", "(cos = cis) 1", "sin = cis; cis 1",
	} {
		if v := value(query); v != "" {
			t.Errorf("%q = %q", query, v)
		}
		if _, ok := RatesOffAnswer(query); ok {
			t.Errorf("%q needs rates", query)
		}
	}
	for _, comparison := range []string{"1 == 1", "1 != 2", "1 ≠ 2", "1 <= 2", "2 >= 1"} {
		if bindsName(asciiSigns(comparison)) {
			t.Errorf("%q binds a name", comparison)
		}
	}
	// `===` is `==` and a lone `=`.
	if !bindsName("1 === 1") {
		t.Error("1 === 1 binds no name")
	}
}

func TestRefusesInputThatIsTooLongOrTooDeep(t *testing.T) {
	for _, query := range []string{
		"1" + strings.Repeat(")", 5000),
		strings.Repeat("-", 5000) + "1",
		strings.Repeat("1+", 300) + "1",
		"100" + strings.Repeat(" - 1%", 100_000),
		// 33 parentheses, and 501 characters.
		strings.Repeat("(", 17) + "10% of 200" + strings.Repeat(")", 16),
		"1 +" + strings.Repeat("　", 497) + "1",
		// Short, but its rewrite nests too deep.
		"100" + strings.Repeat(" - 1%", 70),
		strings.Repeat("1(", 30),
		strings.Repeat("1+1+1+1+1+1+1+(", 8),
		strings.Repeat(strings.Repeat("1−", 15)+"(", 7),
		strings.Repeat("1 2/(", 16),
		strings.Repeat("1 2^-(", 16),
		strings.Repeat("(2/", 30),
		strings.Repeat("1 \\x.1+(", 11),
		strings.Repeat("1 λx.", 30) + "1",
		"2^99999",
		"1e99999",
	} {
		if v := value(query); v != "" {
			t.Errorf("%.40q = %q", query, v)
		}
		if _, ok := RatesOffAnswer(query); ok {
			t.Errorf("%.40q needs rates", query)
		}
	}
	// What people type still has an answer.
	expect(t, value, map[string]string{
		strings.Repeat("1.5+", 40) + "1":                                 "61",
		"(((2+3)*(4-1)+1)/(5-(2+1)))":                                    "8",
		"((1+2)*(3+4))*((5+6)-(7+8))":                                    "-84",
		"1 - (2 - (3 - (4 - (5 - (6 - 7)))))":                            "4",
		"1-(1-(1-(1-(1-(1-1)))))":                                        "1",
		"1 + 1/(1 + 1/(1 + 1/(1 + 1/(1 + 1/(1 + 1)))))":                  "approx. 1.6153846154",
		"200 - 10% - 5% - 2% - 1%":                                       "165.9042",
		"200 - 1% - 1% - 1% - 1% - 1% - 1%":                              "188.2960298802",
		strings.Repeat("(", 16) + "10% of 200" + strings.Repeat(")", 16): "20",
		// 499 characters, but 1,489 bytes.
		"1 +" + strings.Repeat("　", 495) + "1": "2",
	})
}

func TestReadsOperatorWordsAndEuroSigns(t *testing.T) {
	expect(t, value, map[string]string{
		"10 plus 5":            "15",
		"10 minus 4":           "6",
		"10 Times 3":           "30",
		"3 multiplied by 4":    "12",
		"1,000 divided by 200": "5",
		"30 plus 20%":          "36",
	})
	expectNone(t, value, "10 divided 4")
	for query, want := range map[string]string{"€20 + €5k": "20 EUR + 5k EUR", "€ and €x": "€ and €x"} {
		if got := euroAmounts(query); got != want {
			t.Errorf("euroAmounts(%q) = %q", query, got)
		}
	}
	if v, used, _, _ := evaluateQuery("€20 + 10%", fixture); !used || v != "22.00 EUR" {
		t.Errorf("€20 + 10%% = %q, used rates: %v", v, used)
	}
	if _, ok := RatesOffAnswer("€20 to usd"); !ok {
		t.Error("€20 to usd needs rates")
	}
}

func TestRoundsMoneyToCents(t *testing.T) {
	for in, want := range map[string]string{
		"approx. 401.7094017094 MYR": "401.71 MYR",
		"400 MYR":                    "400.00 MYR",
		"0.5 EUR":                    "0.500 EUR",
		"0 EUR":                      "0.00 EUR",
		"-0.004 EUR":                 "-0.00400 EUR",
		"0.001 EUR":                  "0.00100 EUR",
		"MYR":                        "",
		"401 83/117 MYR":             "",
	} {
		if got, _ := money(in); got != want {
			t.Errorf("money(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestDropsLongRoundedAnswersAsTheRustAppDid(t *testing.T) {
	expectNone(t, value,
		"floor(1e20)", "ceil(1e25)", "round(123456789012345678901.4)", "floor(-1e20)", "FLOOR(1e20 m)",
		"floor(10000000000000000001)", "round(9007199254740993)", "floor(12345678901234567.5)",
		"floor(1e20) + 1", "floor(1e20) * 2", "round(10000000000000001)",
	)
	expect(t, value, map[string]string{
		"round(1234567890123.4)": "1234567890123",
		"floor(3.5 km)":          "3 km",
		"ceil(-2.5)":             "-2",
		"round(-2.5)":            "-3",
		"round(2.5)":             "3",
	})
}

func TestKeepsExactCurrencyFractions(t *testing.T) {
	get := func(q string) string { return valueWith(fixture, q) }
	expect(t, get, map[string]string{
		"100 USD to MYR in exact":    "401 83/117 MYR",
		"100 USD to MYR in fraction": "47000/117 MYR",
		// A unit with a `/` is rounded.
		"10 USD/kg to MYR/kg":         "40.17 MYR / kg",
		"10 USD per kg to MYR per kg": "40.17 MYR / kg",
	})
}

func TestKeepsSignificantDigitsOfSmallAmounts(t *testing.T) {
	rates := testRates{"2026-10-02", map[string]float64{"USD": 1.17, "IDR": 18850.0, "JPY": 175.5}}
	expect(t, func(q string) string { return valueWith(rates, q) }, map[string]string{
		"1 IDR to EUR":   "0.0000531 EUR",
		"1 JPY to USD":   "0.00667 USD",
		"100 USD to EUR": "85.47 EUR",
	})
}

func TestFormatsNumbersAsFendDoes(t *testing.T) {
	for _, c := range []struct {
		value string
		style style
		want  string
	}{
		{"2/3", styleAuto, "approx. 0.6666666667"},
		{"-1/3", styleAuto, "approx. -0.3333333333"},
		{"1/30000000000000", styleAuto, "approx. 0"},
		{"5/2", styleExact, "2.5"},
		{"7/3", styleExact, "2 1/3"},
		{"-1/3", styleExact, "-1/3"},
		{"5/2", styleFraction, "5/2"},
		{"123456789/1000", styleAuto, "123456.789"},
	} {
		q := plain(rat(c.value))
		q.style = c.style
		if got := q.String(); got != c.want {
			t.Errorf("%s as %d = %q, want %q", c.value, c.style, got, c.want)
		}
	}
}
