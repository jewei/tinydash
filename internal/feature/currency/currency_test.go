package currency

import (
	"strings"
	"testing"
	"time"
)

func fixture(t *testing.T) Rates {
	t.Helper()
	rates, err := Parse(strings.NewReader(`{"amount":1.0,"base":"EUR","date":"2026-10-02","rates":{"USD":1.17,"MYR":4.7,"CNY":8.0}}`), time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	return rates
}

func TestParsesAndChecksRates(t *testing.T) {
	rates := fixture(t)
	for code, want := range map[string]float64{"EUR": 1, "MYR": 4.7} {
		if got, ok := rates.PerEuro(code); !ok || got != want {
			t.Errorf("PerEuro(%s) = %v, %v; want %v", code, got, ok, want)
		}
	}
	if _, ok := rates.PerEuro("XYZ"); ok {
		t.Error("XYZ has a rate")
	}
	if rates.Currencies() != 4 || rates.PublishedOn() != "2026-10-02" {
		t.Errorf("%d currencies from %s", rates.Currencies(), rates.PublishedOn())
	}
	maxAge := int64(MaxAge / time.Second)
	if rates.IsStale(time.Unix(maxAge-1, 0)) {
		t.Error("stale before 12 hours")
	}
	if !rates.IsStale(time.Unix(maxAge, 0)) {
		t.Error("fresh after 12 hours")
	}
	if !rates.IsStale(time.Unix(-1, 0)) {
		t.Error("rates from a clock that was ahead are fresh")
	}
	for _, bad := range []string{
		`{"base":"USD","date":"2026-10-02","rates":{"EUR":1.0}}`,
		`{"base":"EUR","date":"yesterday","rates":{"USD":1.0}}`,
		`{"base":"EUR","date":"2026-10-02","rates":{"usd":1.0}}`,
		`{"base":"EUR","date":"2026-10-02","rates":{"USD":-1.0}}`,
		`{"base":"EUR","date":"2026-10-02","rates":{}}`,
		`{"base":"EUR","date":"2026-10-02"`,
	} {
		if _, err := Parse(strings.NewReader(bad), time.Unix(0, 0)); err == nil {
			t.Errorf("%s parsed", bad)
		}
	}
}
