// Package store is the SQLite database, tinydash.db: the only package that
// knows the schema or runs SQL. Usage, pins, hidden results, aliases,
// hotkeys, clipboard history, the snippet library, copied calculations, and
// cached exchange rates and weather live there. The note table of the Rust
// app stays in the file untouched, so going back to it loses nothing.
package store

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/egoist/mygo/plugins/sqlite"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/library"
)

const (
	FileName = "tinydash.db"
	// maxUsage is the most usage rows kept (MAX_USAGE of the Rust app's
	// usage ranking): the least recently used go first.
	maxUsage = 1000
	// maxCalculations is the most copied calculations kept
	// (MAX_CALCULATIONS of the Rust calculator).
	maxCalculations = 50
	busyTimeout     = 2 * time.Second
)

// Store is an open tinydash.db. Its methods are safe from any goroutine;
// each one is a single statement or one transaction.
type Store struct {
	db *sqlite.DB
}

// Calculation is a copied calculation, kept for the calculator history.
type Calculation struct {
	ID         int64
	Expression string
	Result     string
}

// Open opens tinydash.db in dir, creating both when missing, and upgrades
// its schema. restrict makes the file readable only by the user, which is
// OS work. When it fails, the store still opens, because losing every pin
// and snippet would be worse, and the warning says what to do.
func Open(ctx context.Context, dir string, restrict func(path string) error) (*Store, string, error) {
	if err := os.MkdirAll(dir, 0o777); err != nil {
		return nil, "", fileError{err}
	}
	path := filepath.Join(dir, FileName)
	s, err := open(ctx, path)
	if err != nil {
		return nil, "", err
	}
	warning := ""
	// Clipboard history can hold private text.
	if err := restrict(path); err != nil {
		warning = fmt.Sprintf("TinyDash could not make %s private to your user account (%v). Other accounts on this computer may read your clipboard history and snippets: make the file readable only by you.", path, err)
	}
	return s, warning, nil
}

// OpenMemory opens a private database that lives in memory, for tests or
// when the real one cannot open.
func OpenMemory(ctx context.Context) (*Store, error) { return open(ctx, ":memory:") }

func open(ctx context.Context, path string) (*Store, error) {
	db, err := sqlite.Open(ctx, path, sqlite.OpenOptions{BusyTimeout: busyTimeout})
	if err != nil {
		return nil, dataError(err)
	}
	// Deleted clipboard rows are overwritten on disk, not just unlinked.
	if _, err = db.Execute(ctx, "PRAGMA secure_delete = ON"); err == nil {
		err = migrate(ctx, db)
	}
	if err != nil {
		return nil, errors.Join(err, db.Close())
	}
	return &Store{db: db}, nil
}

// Close closes the database.
func (s *Store) Close() error { return s.db.Close() }

func migrate(ctx context.Context, db *sqlite.DB) error {
	rows, err := db.Query(ctx, "PRAGMA user_version")
	if err != nil {
		return dataError(err)
	}
	version, err := integer(rows.Values[0][0])
	if err != nil {
		return dataError(err)
	}
	if version < 0 || version > int64(len(migrations)) {
		return message("The database was created by a newer TinyDash. Update TinyDash to use it.")
	}
	for i := int(version); i < len(migrations); i++ {
		var batch []sqlite.Statement
		for _, sql := range statements(migrations[i]) {
			batch = append(batch, sqlite.Statement{SQL: sql})
		}
		batch = append(batch, sqlite.Statement{SQL: fmt.Sprintf("PRAGMA user_version = %d", i+1)})
		if _, err := db.Transaction(ctx, batch); err != nil {
			return dataError(err)
		}
	}
	return nil
}

func (s *Store) exec(ctx context.Context, sql string, args ...any) error {
	_, err := s.db.Execute(ctx, sql, args...)
	return dataError(err)
}

func (s *Store) transaction(ctx context.Context, batch ...sqlite.Statement) error {
	_, err := s.db.Transaction(ctx, batch)
	return dataError(err)
}

// query runs sql and calls scan for each row; a scan error stops it.
func (s *Store) query(ctx context.Context, sql string, args []any, scan func(row []any) error) error {
	rows, err := s.db.Query(ctx, sql, args...)
	if err != nil {
		return dataError(err)
	}
	for _, row := range rows.Values {
		if err := scan(row); err != nil {
			return dataError(err)
		}
	}
	return nil
}

// Usage is how often and how lately each result ran.
func (s *Store) Usage(ctx context.Context) (map[core.ID]core.Use, error) {
	usage := map[core.ID]core.Use{}
	err := s.query(ctx, "SELECT id, count, last_used FROM usage", nil, func(row []any) error {
		id, err := text(row[0])
		if err != nil {
			return err
		}
		count, err := integer(row[1])
		if err != nil {
			return err
		}
		if count < 0 || count > int64(^uint32(0)) {
			return fmt.Errorf("usage count %d is out of range", count)
		}
		lastUsed, err := integer(row[2])
		if err != nil {
			return err
		}
		usage[core.ID(id)] = core.Use{Count: uint32(count), LastUsed: lastUsed}
		return nil
	})
	return usage, err
}

// RecordUse saves one result's usage, keeping only the most recently used
// rows. Two uses at once may save in either order, so a row never goes
// back.
func (s *Store) RecordUse(ctx context.Context, id core.ID, used core.Use) error {
	return s.transaction(ctx,
		sqlite.Statement{SQL: `INSERT INTO usage (id, count, last_used) VALUES (?1, ?2, ?3)
			ON CONFLICT (id) DO UPDATE
			SET count = max(count, ?2), last_used = max(last_used, ?3)`,
			Args: []any{string(id), used.Count, used.LastUsed}},
		sqlite.Statement{SQL: `DELETE FROM usage WHERE id NOT IN
			(SELECT id FROM usage ORDER BY last_used DESC, id LIMIT ?1)`,
			Args: []any{maxUsage}},
	)
}

// Pins are the pinned result IDs, in pin order.
func (s *Store) Pins(ctx context.Context) ([]core.ID, error) {
	return s.ids(ctx, "SELECT id FROM pins ORDER BY position")
}

func (s *Store) SetPinned(ctx context.Context, id core.ID, pinned bool) error {
	if pinned {
		return s.exec(ctx, `INSERT OR IGNORE INTO pins (id, position)
			VALUES (?1, (SELECT COALESCE(MAX(position), 0) + 1 FROM pins))`, string(id))
	}
	return s.exec(ctx, "DELETE FROM pins WHERE id = ?1", string(id))
}

// ClipboardHistory is the saved entries, newest first, without image bytes.
func (s *Store) ClipboardHistory(ctx context.Context) ([]clipboard.Entry, error) {
	var entries []clipboard.Entry
	err := s.query(ctx, "SELECT id, kind, title, content, copied_at FROM clipboard ORDER BY copied_at DESC, id DESC", nil, func(row []any) error {
		var e clipboard.Entry
		var err error
		if e.ID, err = integer(row[0]); err != nil {
			return err
		}
		if e.Kind, err = clipKind(row[1]); err != nil {
			return err
		}
		if e.Title, err = text(row[2]); err != nil {
			return err
		}
		if e.Text, err = text(row[3]); err != nil {
			return err
		}
		if e.CopiedAt, err = integer(row[4]); err != nil {
			return err
		}
		// File lists are stored as JSON; search their plain paths.
		if e.Kind == clipboard.KindFiles {
			if paths, err := decodePaths(e.Text); err == nil {
				e.Text = strings.Join(paths, "\n")
			}
		}
		entries = append(entries, e)
		return nil
	})
	return entries, err
}

// SaveClip saves a capture. A repeat moves the existing entry to the top.
// Then the oldest unpinned entries beyond limit, and unpinned images beyond
// clipboard.MaxImages, go.
//
// Entries sort by copied_at, then by ID. A repeat keeps its older ID, so it
// must be newer than every other entry: a copy in the same second as the
// one before it would otherwise stay below it. A new entry, with the
// highest ID, needs only a tie. Both times are worked out in the INSERT, so
// they read the table inside the transaction.
func (s *Store) SaveClip(ctx context.Context, content clipboard.Content, now int64, limit uint32) error {
	var (
		body          string
		image         any
		width, height any
	)
	switch content.Kind {
	case clipboard.KindText:
		body = content.Text
	case clipboard.KindImage:
		image, width, height = content.PNG, content.Width, content.Height
		if content.PNG == nil {
			image = []byte{}
		}
	case clipboard.KindFiles:
		paths, err := encodePaths(content.Files)
		if err != nil {
			return err
		}
		body = paths
	default:
		return fmt.Errorf("clipboard content of unknown kind %q", content.Kind)
	}
	return s.transaction(ctx, append([]sqlite.Statement{{
		SQL: `INSERT INTO clipboard (kind, hash, title, content, image, width, height, copied_at)
			VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
				max(?8, coalesce((SELECT MAX(copied_at) FROM clipboard WHERE hash <> ?2), ?8)))
			ON CONFLICT (hash) DO UPDATE SET copied_at =
				max(?8, coalesce((SELECT MAX(copied_at) FROM clipboard WHERE hash <> ?2) + 1, ?8))`,
		Args: []any{string(content.Kind), content.Hash(), content.Title(), body, image, width, height, now},
	}}, trim(limit)...)...)
}

// TrimClipboard deletes the oldest unpinned entries beyond limit, for
// example after the user lowered it.
func (s *Store) TrimClipboard(ctx context.Context, limit uint32) error {
	return s.transaction(ctx, trim(limit)...)
}

const unpinned = "'clip:' || id NOT IN (SELECT id FROM pins)"

func trim(limit uint32) []sqlite.Statement {
	return []sqlite.Statement{
		{SQL: `DELETE FROM clipboard WHERE id IN (SELECT id FROM clipboard WHERE ` + unpinned + `
			ORDER BY copied_at DESC, id DESC LIMIT -1 OFFSET ?1)`, Args: []any{limit}},
		{SQL: `DELETE FROM clipboard WHERE id IN (SELECT id FROM clipboard WHERE kind = 'image'
			AND ` + unpinned + ` ORDER BY copied_at DESC, id DESC LIMIT -1 OFFSET ?1)`, Args: []any{clipboard.MaxImages}},
	}
}

// Clip is the full saved content of one entry; ok is false when there is
// none.
func (s *Store) Clip(ctx context.Context, id int64) (content clipboard.Content, ok bool, err error) {
	err = s.query(ctx, "SELECT kind, content, image, width, height FROM clipboard WHERE id = ?1", []any{id}, func(row []any) error {
		ok = true
		kind, err := clipKind(row[0])
		if err != nil {
			return err
		}
		body, err := text(row[1])
		if err != nil {
			return err
		}
		switch kind {
		case clipboard.KindText:
			content = clipboard.Text(body)
		case clipboard.KindFiles:
			paths, err := decodePaths(body)
			if err != nil {
				return err
			}
			content = clipboard.Files(paths)
		case clipboard.KindImage:
			png, _ := row[2].([]byte)
			width, err := optionalUint32(row[3])
			if err != nil {
				return err
			}
			height, err := optionalUint32(row[4])
			if err != nil {
				return err
			}
			content = clipboard.Image(png, width, height)
		}
		return nil
	})
	if err != nil {
		return clipboard.Content{}, false, err
	}
	return content, ok, nil
}

func (s *Store) DeleteClip(ctx context.Context, id int64) error {
	return s.transaction(ctx,
		sqlite.Statement{SQL: "DELETE FROM clipboard WHERE id = ?1", Args: []any{id}},
		sqlite.Statement{SQL: "DELETE FROM pins WHERE id = ?1", Args: []any{string(clipID(id))}},
	)
}

// ClearClipboard removes every entry except pinned ones.
func (s *Store) ClearClipboard(ctx context.Context) error {
	return s.exec(ctx, "DELETE FROM clipboard WHERE "+unpinned)
}

func clipID(id int64) core.ID { return core.SourceClip.ID(strconv.FormatInt(id, 10)) }

// Library is every saved snippet and quicklink.
func (s *Store) Library(ctx context.Context) ([]library.Item, error) {
	var items []library.Item
	err := s.query(ctx, "SELECT id, kind, name, keyword, text FROM library", nil, func(row []any) error {
		var item library.Item
		var err error
		if item.ID, err = integer(row[0]); err != nil {
			return err
		}
		kind, err := text(row[1])
		if err != nil {
			return err
		}
		switch item.Kind = library.Kind(kind); item.Kind {
		case library.KindSnippet, library.KindQuicklink:
		default:
			return fmt.Errorf("library item of unknown kind %q", kind)
		}
		for i, field := range []*string{&item.Name, &item.Keyword, &item.Text} {
			if *field, err = text(row[2+i]); err != nil {
				return err
			}
		}
		items = append(items, item)
		return nil
	})
	return items, err
}

// SaveLibraryItem inserts an item without an ID or updates the one with
// its ID, and returns it with its ID. Updating an item that was deleted
// meanwhile fails instead of saving nothing.
func (s *Store) SaveLibraryItem(ctx context.Context, item library.Item) (library.Item, error) {
	var result sqlite.Result
	var err error
	if item.ID != 0 {
		result, err = s.db.Execute(ctx, "UPDATE library SET kind = ?2, name = ?3, keyword = ?4, text = ?5 WHERE id = ?1",
			item.ID, string(item.Kind), item.Name, item.Keyword, item.Text)
	} else {
		result, err = s.db.Execute(ctx, "INSERT INTO library (kind, name, keyword, text) VALUES (?1, ?2, ?3, ?4)",
			string(item.Kind), item.Name, item.Keyword, item.Text)
		item.ID = result.LastInsertID
	}
	if err != nil {
		return library.Item{}, dataError(err)
	}
	if result.Changes == 0 {
		return library.Item{}, message("This item was deleted, so it was not saved. Save again to add it as a new item.")
	}
	return item, nil
}

// DeleteLibraryItem deletes an item with its pin, usage, alias, hotkey,
// and hidden mark, under either result ID it may have had.
func (s *Store) DeleteLibraryItem(ctx context.Context, id int64) error {
	key := strconv.FormatInt(id, 10)
	ids := []any{string(core.SourceSnippet.ID(key)), string(core.SourceLink.ID(key))}
	batch := []sqlite.Statement{{SQL: "DELETE FROM library WHERE id = ?1", Args: []any{id}}}
	for _, table := range []string{"pins", "usage", "aliases", "hotkeys", "hidden"} {
		batch = append(batch, sqlite.Statement{SQL: "DELETE FROM " + table + " WHERE id IN (?1, ?2)", Args: ids})
	}
	return s.transaction(ctx, batch...)
}

// Hidden is the result IDs the user hid, oldest first.
func (s *Store) Hidden(ctx context.Context) ([]core.ID, error) {
	return s.ids(ctx, "SELECT id FROM hidden ORDER BY hidden_at, id")
}

func (s *Store) SetHidden(ctx context.Context, id core.ID, hidden bool, now int64) error {
	if hidden {
		return s.exec(ctx, `INSERT INTO hidden (id, hidden_at) VALUES (?1, ?2)
			ON CONFLICT (id) DO NOTHING`, string(id), now)
	}
	return s.exec(ctx, "DELETE FROM hidden WHERE id = ?1", string(id))
}

// Aliases maps result IDs to the alias the user gave them.
func (s *Store) Aliases(ctx context.Context) (map[core.ID]string, error) {
	return s.pairs(ctx, "SELECT id, alias FROM aliases")
}

// SetAlias gives a result an alias, replacing its old one. It fails when
// another result has the alias.
func (s *Store) SetAlias(ctx context.Context, id core.ID, alias string) error {
	return s.exec(ctx, `INSERT INTO aliases (id, alias) VALUES (?1, ?2)
		ON CONFLICT (id) DO UPDATE SET alias = ?2`, string(id), alias)
}

func (s *Store) RemoveAlias(ctx context.Context, id core.ID) error {
	return s.exec(ctx, "DELETE FROM aliases WHERE id = ?1", string(id))
}

// Hotkeys maps result IDs to their shortcut, in Tauri accelerator syntax.
func (s *Store) Hotkeys(ctx context.Context) (map[core.ID]string, error) {
	return s.pairs(ctx, "SELECT id, shortcut FROM hotkeys")
}

// SetHotkey gives a result a hotkey, replacing its old one. It fails when
// another result has the hotkey.
func (s *Store) SetHotkey(ctx context.Context, id core.ID, shortcut string) error {
	return s.exec(ctx, `INSERT INTO hotkeys (id, shortcut) VALUES (?1, ?2)
		ON CONFLICT (id) DO UPDATE SET shortcut = ?2`, string(id), shortcut)
}

func (s *Store) RemoveHotkey(ctx context.Context, id core.ID) error {
	return s.exec(ctx, "DELETE FROM hotkeys WHERE id = ?1", string(id))
}

// Calculations is the copied calculations, newest first.
func (s *Store) Calculations(ctx context.Context) ([]Calculation, error) {
	var out []Calculation
	err := s.query(ctx, "SELECT id, expression, result FROM calculations ORDER BY used_at DESC, id DESC", nil, func(row []any) error {
		var c Calculation
		var err error
		if c.ID, err = integer(row[0]); err != nil {
			return err
		}
		if c.Expression, err = text(row[1]); err != nil {
			return err
		}
		if c.Result, err = text(row[2]); err != nil {
			return err
		}
		out = append(out, c)
		return nil
	})
	return out, err
}

// SaveCalculation keeps a copied calculation. A repeat moves to the top
// with its new result; the oldest beyond the limit go. A repeat gets a new,
// highest ID, so it is on top even when an earlier copy in the same second
// has the same used_at.
func (s *Store) SaveCalculation(ctx context.Context, expression, result string, now int64) error {
	return s.transaction(ctx,
		sqlite.Statement{SQL: "DELETE FROM calculations WHERE expression = ?1", Args: []any{expression}},
		sqlite.Statement{SQL: "INSERT INTO calculations (expression, result, used_at) VALUES (?1, ?2, ?3)",
			Args: []any{expression, result, now}},
		sqlite.Statement{SQL: `DELETE FROM calculations WHERE id IN (SELECT id FROM calculations
			ORDER BY used_at DESC, id DESC LIMIT -1 OFFSET ?1)`, Args: []any{maxCalculations}},
	)
}

func (s *Store) DeleteCalculation(ctx context.Context, id int64) error {
	return s.exec(ctx, "DELETE FROM calculations WHERE id = ?1", id)
}

func (s *Store) ClearCalculations(ctx context.Context) error {
	return s.exec(ctx, "DELETE FROM calculations")
}

// The keys of the cache table, the Rust app's.
const (
	ratesKey   = "currency_rates"
	weatherKey = "weather"
)

// Rates is the cached exchange rates as the currency feature wrote them
// (JSON); ok is false when none are cached.
func (s *Store) Rates(ctx context.Context) (rates string, ok bool, err error) {
	return s.cached(ctx, ratesKey)
}

func (s *Store) SaveRates(ctx context.Context, rates string) error {
	return s.saveCached(ctx, ratesKey, rates)
}

// Weather is the latest weather download as the weather feature wrote it
// (JSON); ok is false when none is cached.
func (s *Store) Weather(ctx context.Context) (weather string, ok bool, err error) {
	return s.cached(ctx, weatherKey)
}

func (s *Store) SaveWeather(ctx context.Context, weather string) error {
	return s.saveCached(ctx, weatherKey, weather)
}

func (s *Store) cached(ctx context.Context, key string) (value string, ok bool, err error) {
	err = s.query(ctx, "SELECT value FROM cache WHERE key = ?1", []any{key}, func(row []any) error {
		ok = true
		value, err = text(row[0])
		return err
	})
	if err != nil {
		return "", false, err
	}
	return value, ok, nil
}

func (s *Store) saveCached(ctx context.Context, key, value string) error {
	return s.exec(ctx, `INSERT INTO cache (key, value) VALUES (?1, ?2)
		ON CONFLICT (key) DO UPDATE SET value = ?2`, key, value)
}

func (s *Store) DeleteRates(ctx context.Context) error {
	return s.exec(ctx, "DELETE FROM cache WHERE key = ?1", ratesKey)
}

func (s *Store) ids(ctx context.Context, sql string) ([]core.ID, error) {
	var ids []core.ID
	err := s.query(ctx, sql, nil, func(row []any) error {
		id, err := text(row[0])
		if err != nil {
			return err
		}
		ids = append(ids, core.ID(id))
		return nil
	})
	return ids, err
}

func (s *Store) pairs(ctx context.Context, sql string) (map[core.ID]string, error) {
	out := map[core.ID]string{}
	err := s.query(ctx, sql, nil, func(row []any) error {
		id, err := text(row[0])
		if err != nil {
			return err
		}
		out[core.ID(id)], err = text(row[1])
		return err
	})
	return out, err
}
