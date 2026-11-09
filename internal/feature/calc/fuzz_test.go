package calc

import "testing"

// FuzzAnswer feeds the calculator any text, as the search field does on
// every keystroke: it must never panic.
func FuzzAnswer(f *testing.F) {
	for _, seed := range []string{
		"12 * 8", "401 83/117 MYR", "10% of 200 + 20% of 300", "20 is 10% of what", "€20 + €5k",
		"100 USD to MYR in exact", "approx. 401.7094017094 MYR", "8 Mbps to MBps", "sqrt(2) ^ 0.5",
		"1 2/(", "-$5 +", "5 €€", "2^-3", "(0) ^ -1", "floor(-2.5 km)", "100 celsius to °F",
	} {
		f.Add(seed)
	}
	f.Fuzz(func(t *testing.T, query string) {
		Answer(Expression(query), fixture)
		Answer(query, nil)
		RatesOffAnswer(query)
	})
}
