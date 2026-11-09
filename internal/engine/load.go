package engine

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"

	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/feature/emoji"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/store"
)

// Load builds the first snapshot from the saved data. A part that cannot be
// read starts empty and adds a warning for the user; the other parts still
// load, so one broken table never costs every pin and snippet. The apps and
// files start empty and not ready, until their first rebuild. home is the
// user's home folder, and random draws passwords.
func Load(ctx context.Context, db *store.Store, s settings.Settings, home string, random io.Reader) (search.Snapshot, []string) {
	var warnings []string
	load := func(what string, err error) bool {
		if err != nil {
			warnings = append(warnings, fmt.Sprintf("Could not load %s. %v", what, err))
		}
		return err == nil
	}
	languages := make([]emoji.Language, len(s.EmojiLanguages))
	for i, language := range s.EmojiLanguages {
		languages[i] = emoji.Language(language)
	}
	snapshot := search.Snapshot{
		Settings:  s,
		Apps:      apps.NewIndex(nil),
		Files:     files.Scan(nil, files.Options{}),
		Clipboard: clipboard.NewHistory(nil),
		Library:   library.NewIndex(nil),
		Emoji:     emoji.NewIndex(languages...),
		Home:      home,
		Random:    random,
	}
	history, err := calculations(ctx, db)
	if load("calculator history", err) {
		snapshot.Calculations = history
	}
	if usage, err := db.Usage(ctx); load("usage", err) {
		snapshot.Usage = search.NewUsage(usage)
	}
	if pins, err := db.Pins(ctx); load("pins", err) {
		snapshot.Pins = search.NewPins(pins)
	}
	if hidden, err := db.Hidden(ctx); load("hidden results", err) {
		snapshot.Hidden = search.NewHidden(hidden...)
	}
	if aliases, err := db.Aliases(ctx); load("aliases", err) {
		snapshot.Aliases = search.NewAliases(aliases)
	}
	if hotkeys, err := db.Hotkeys(ctx); load("hotkeys", err) {
		snapshot.Hotkeys = search.NewHotkeys(hotkeys)
	}
	if entries, err := db.ClipboardHistory(ctx); load("clipboard history", err) {
		snapshot.Clipboard = clipboard.NewHistory(entries)
	}
	if items, err := db.Library(ctx); load("snippets", err) {
		snapshot.Library = library.NewIndex(items)
	}
	if s.CurrencyRatesEnabled {
		if rates, ok := loadRates(ctx, db, load); ok {
			snapshot.Rates = &rates
		}
	} else if err := db.DeleteRates(ctx); err != nil {
		// Rates that are off are not kept, even from a version that did.
		slog.Warn("Could not delete the saved exchange rates", "error", err)
	}
	return snapshot, warnings
}

func loadRates(ctx context.Context, db *store.Store, load func(string, error) bool) (currency.Rates, bool) {
	text, ok, err := db.Rates(ctx)
	if !load("exchange rates", err) || !ok {
		return currency.Rates{}, false
	}
	var rates currency.Rates
	if err := json.Unmarshal([]byte(text), &rates); !load("exchange rates", err) {
		return currency.Rates{}, false
	}
	return rates, true
}
