package engine

import (
	"context"
	"errors"
	"math/rand/v2"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

func openStore(t *testing.T) *store.Store {
	t.Helper()
	db, err := store.OpenMemory(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

// savedWeather is weather as the Rust app saved it.
const savedWeather = `{"city":"singapore","place":"Singapore","temperature":31.4,"high":33.2,"low":25.6,"rain_chance":null,"code":2,"is_day":true,"fetched_at":100}`

const savedRates = `{"date":"2026-11-06","fetched_at":1762400000,"rates":{"USD":1.1}}`

func TestLoadReadsEverySavedList(t *testing.T) {
	db := openStore(t)
	ctx := context.Background()
	lock := core.ID("system:lock")
	steps := []error{
		db.RecordUse(ctx, lock, core.Use{Count: 3, LastUsed: testNow.Unix()}),
		db.SetPinned(ctx, lock, true),
		db.SetHidden(ctx, "system:sleep", true, testNow.Unix()),
		db.SetAlias(ctx, lock, "lk"),
		db.SetHotkey(ctx, lock, "Alt+L"),
		db.SaveCalculation(ctx, "1+1", "2", testNow.Unix()),
		db.SaveRates(ctx, savedRates),
		db.SaveWeather(ctx, savedWeather),
	}
	withLibrary(&search.Snapshot{}, db)
	if err := errors.Join(steps...); err != nil {
		t.Fatal(err)
	}
	s := settings.Default()
	s.CurrencyRatesEnabled = true
	first, warnings := Load(ctx, db, s, home, rand.NewChaCha8([32]byte{}))
	if len(warnings) != 0 {
		t.Fatal(warnings)
	}
	alias, _ := first.Aliases.Get(lock)
	hotkey, _ := first.Hotkeys.Get(lock)
	if first.Usage.Bonus(lock, testNow.Unix()) == 0 || !first.Pins.Contains(lock) || !first.Hidden.Contains("system:sleep") ||
		alias != "lk" || hotkey != "Alt+L" || len(first.Library.Items()) != 3 || first.Rates == nil || first.Home != home ||
		first.Weather == nil || first.Weather.Place != "Singapore" {
		t.Errorf("%+v", first)
	}
	if _, ok := first.Calculations.Answer("="); !ok {
		t.Error("calculations")
	}
	if first.AppsReady || first.FilesReady {
		t.Error("ready before the first scan")
	}
	// Every field is set, so search runs on it at once.
	first.Now = testNow
	search.Search(&first, "a", core.CategoryAll)
}

func TestLoadKeepsTheOtherPartsWhenOnePartFails(t *testing.T) {
	db := openStore(t)
	ctx := context.Background()
	if err := errors.Join(db.SetPinned(ctx, "system:lock", true), db.SaveRates(ctx, "not json")); err != nil {
		t.Fatal(err)
	}
	s := settings.Default()
	s.CurrencyRatesEnabled = true
	first, warnings := Load(ctx, db, s, home, nil)
	if len(warnings) != 1 || !strings.HasPrefix(warnings[0], "Could not load exchange rates. ") {
		t.Errorf("%q", warnings)
	}
	if first.Rates != nil || !first.Pins.Contains("system:lock") {
		t.Error("one broken part cost the others")
	}
}

func TestLoadWarnsForEachPartOfAnUnreadableDatabase(t *testing.T) {
	db := openStore(t)
	db.Close()
	s := settings.Default()
	s.CurrencyRatesEnabled = true
	first, warnings := Load(context.Background(), db, s, home, nil)
	var parts []string
	for _, w := range warnings {
		what, _, _ := strings.Cut(strings.TrimPrefix(w, "Could not load "), ".")
		parts = append(parts, what)
	}
	want := []string{"calculator history", "usage", "pins", "hidden results", "aliases", "hotkeys", "clipboard history", "snippets", "exchange rates", "the weather"}
	if !slices.Equal(parts, want) {
		t.Errorf("%q", warnings)
	}
	first.Now = testNow
	search.Search(&first, "a", core.CategoryAll)
}

func TestRatesThatAreOffAreNotKept(t *testing.T) {
	db := openStore(t)
	ctx := context.Background()
	if err := db.SaveRates(ctx, savedRates); err != nil {
		t.Fatal(err)
	}
	first, _ := Load(ctx, db, settings.Default(), home, nil)
	if _, ok, _ := db.Rates(ctx); ok || first.Rates != nil {
		t.Error("rates kept while off")
	}
}

func TestRebuildsPublishTheIndexAndTheFirstMarksItReady(t *testing.T) {
	dir := t.TempDir()
	notes := filepath.Join(dir, "notes.txt")
	if err := os.WriteFile(notes, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.AppsReady, first.FilesReady = false, false
		first.Settings.FileSearchFolders = []string{dir}
	})
	ctx := context.Background()
	if w.snapshot().Files.Contains(notes) {
		t.Fatal("indexed before the scan")
	}
	w.engine.RebuildFiles(FileDisk{Disk: osDisk{}})(ctx)
	if s := w.snapshot(); !s.FilesReady || !s.Files.Contains(notes) || s.AppsReady {
		t.Error("the file scan")
	}

	other := golden.Abs("/Applications/Other.app")
	w.engine.RebuildApps(func() ([]apps.App, error) {
		return []apps.App{{Name: "Other", Path: other}}, nil
	})(ctx)
	if s := w.snapshot(); !s.AppsReady || !s.Apps.Contains(other) || s.Apps.Contains(safari) {
		t.Error("the app list")
	}
	// A failed listing keeps the apps found before.
	w.engine.RebuildApps(func() ([]apps.App, error) { return nil, errors.New("no") })(ctx)
	if !w.snapshot().Apps.Contains(other) {
		t.Error("a failed listing emptied the apps")
	}

	// A slot runs the rebuild; it is ready once search sees the index.
	slot := NewSlot(w.engine.RebuildApps(func() ([]apps.App, error) { return nil, nil }))
	run, stop := context.WithCancel(ctx)
	defer stop()
	go slot.Run(run)
	slot.Request()
	for !slot.Ready() {
		runtime.Gosched()
	}
	if w.snapshot().Apps.Len() != 0 {
		t.Error("the slot's rebuild is not published")
	}
}
