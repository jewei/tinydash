// Package currency holds the daily ECB exchange rates from Frankfurter. Only
// the rate table is downloaded; queries and amounts never leave the machine.
// The download itself is the engine's job; this package parses and checks it.
package currency

import (
	"encoding/json"
	"errors"
	"io"
	"time"
)

// MaxResponseBytes is the most of a Frankfurter response that Parse reads.
const MaxResponseBytes = 64 * 1024

// MaxAge is how old rates may be before the launcher refreshes them.
const MaxAge = 12 * time.Hour

// Rates is a Frankfurter `latest` table. The JSON names are the ones that
// the Rust app saved in tinydash.db, so a saved table loads in both.
type Rates struct {
	// Date is the publication date, YYYY-MM-DD.
	Date string `json:"date"`
	// FetchedAt is the download time in Unix seconds.
	FetchedAt int64 `json:"fetched_at"`
	// Table holds the units of each currency per euro.
	Table map[string]float64 `json:"rates"`
}

var errUnexpected = errors.New("the exchange rate service sent unexpected data")

// Parse reads and checks a Frankfurter `latest` response downloaded at now.
func Parse(r io.Reader, now time.Time) (Rates, error) {
	var response struct {
		Base  string             `json:"base"`
		Date  string             `json:"date"`
		Rates map[string]float64 `json:"rates"`
	}
	if err := json.NewDecoder(io.LimitReader(r, MaxResponseBytes)).Decode(&response); err != nil {
		return Rates{}, err
	}
	_, dateErr := time.Parse(time.DateOnly, response.Date)
	if response.Base != "EUR" || dateErr != nil || len(response.Rates) == 0 {
		return Rates{}, errUnexpected
	}
	for code, rate := range response.Rates {
		// JSON has no infinity or NaN, so a positive rate is a usable one.
		if !isCode(code) || rate <= 0 {
			return Rates{}, errUnexpected
		}
	}
	return Rates{Date: response.Date, FetchedAt: now.Unix(), Table: response.Rates}, nil
}

// isCode reports whether code has the shape of an ISO 4217 code: three
// ASCII capital letters.
func isCode(code string) bool {
	if len(code) != 3 {
		return false
	}
	for i := range len(code) {
		if code[i] < 'A' || code[i] > 'Z' {
			return false
		}
	}
	return true
}

// PerEuro returns how many units of the currency one euro buys.
func (r Rates) PerEuro(code string) (float64, bool) {
	if code == "EUR" {
		return 1, true
	}
	rate, ok := r.Table[code]
	return rate, ok
}

// Currencies counts the currencies of the table, the euro included.
func (r Rates) Currencies() int { return len(r.Table) + 1 }

// PublishedOn is the publication date, YYYY-MM-DD.
func (r Rates) PublishedOn() string { return r.Date }

// IsStale reports whether the rates need a refresh. Rates saved in the future
// came from a clock that was ahead; they are stale too, or they would stay
// fresh until the clock catches up.
func (r Rates) IsStale(now time.Time) bool {
	age := now.Unix() - r.FetchedAt
	return age < 0 || age >= int64(MaxAge/time.Second)
}
