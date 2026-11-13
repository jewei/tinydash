package engine

import (
	"context"
	"encoding/json"
	"reflect"
	"testing"

	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

var downloaded = currency.Rates{Date: "2026-11-12", FetchedAt: 100, Table: map[string]float64{"USD": 1.16, "MYR": 4.9}}

func ratesOn(first *search.Snapshot, _ *store.Store) { first.Settings.CurrencyRatesEnabled = true }

// Rates download only after the user opts in, so a download that ends
// while they are off is dropped without a word.
func TestDropsRatesWhileTheyAreOff(t *testing.T) {
	w := newWorld(t, nil)
	if err := <-w.engine.SetRates(downloaded); err != nil {
		t.Fatal(err)
	}
	if _, ok, _ := w.db.Rates(context.Background()); ok || w.snapshot().Rates != nil {
		t.Error("kept rates that are off")
	}
}

// Saved rates load back as they were downloaded, so the next start can
// convert before it downloads again.
func TestSavesAndPublishesRatesWhileTheyAreOn(t *testing.T) {
	w := newWorld(t, ratesOn)
	if err := <-w.engine.SetRates(downloaded); err != nil {
		t.Fatal(err)
	}
	if got := w.snapshot().Rates; got == nil || !reflect.DeepEqual(*got, downloaded) {
		t.Fatalf("published %+v", got)
	}
	saved, ok, err := w.db.Rates(context.Background())
	var back currency.Rates
	if !ok || err != nil || json.Unmarshal([]byte(saved), &back) != nil || !reflect.DeepEqual(back, downloaded) {
		t.Fatalf("saved %q, %v, %v", saved, ok, err)
	}
}

// The check runs on the writer after the settings change queued before
// it, so a download that ends just after the user turns rates off never
// comes back.
func TestDropsRatesThatEndAfterTheUserTurnsThemOff(t *testing.T) {
	w := newWorld(t, ratesOn)
	_, _, file := LoadSettings(t.TempDir())
	off := w.engine.UpdateSettings(file, func(s *settings.Settings) { s.CurrencyRatesEnabled = false })
	late := w.engine.SetRates(downloaded)
	if err := <-off; err != nil {
		t.Fatal(err)
	}
	if err := <-late; err != nil {
		t.Fatal(err)
	}
	if _, ok, _ := w.db.Rates(context.Background()); ok || w.snapshot().Rates != nil {
		t.Error("kept rates that ended after the user turned them off")
	}
}

// A failed save publishes nothing: the launcher keeps the rates it had,
// which match what the next start loads.
func TestKeepsTheOldRatesWhenTheSaveFails(t *testing.T) {
	old := currency.Rates{Date: "2026-11-11", Table: map[string]float64{"USD": 1.15}}
	w := newWorld(t, func(first *search.Snapshot, db *store.Store) {
		ratesOn(first, db)
		first.Rates = &old
	})
	if err := w.db.Close(); err != nil {
		t.Fatal(err)
	}
	if err := <-w.engine.SetRates(downloaded); err == nil {
		t.Fatal("a failed save reported no error")
	}
	if got := w.snapshot().Rates; got == nil || !reflect.DeepEqual(*got, old) {
		t.Errorf("published %+v after a failed save", got)
	}
}
