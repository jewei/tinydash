package engine

import (
	"context"
	"encoding/json"

	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// SetRates saves downloaded exchange rates and publishes them, unless the
// user turned rates off while they downloaded.
func (e *Engine) SetRates(rates currency.Rates) <-chan error {
	return e.Change(func(next *search.Snapshot, db *store.Store) error {
		if !next.Settings.CurrencyRatesEnabled {
			return errUnchanged
		}
		data, err := json.Marshal(rates)
		if err != nil {
			return err
		}
		if err := db.SaveRates(context.Background(), string(data)); err != nil {
			return err
		}
		next.Rates = &rates
		return nil
	})
}
