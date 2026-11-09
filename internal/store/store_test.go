package store

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"strconv"
	"strings"
	"testing"

	"github.com/egoist/mygo/plugins/sqlite"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/library"
)

var ctx = context.Background()

func memory(t *testing.T) *Store {
	t.Helper()
	s, err := OpenMemory(ctx)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { s.Close() })
	return s
}

func must(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}

func titles(t *testing.T, s *Store) []string {
	t.Helper()
	history, err := s.ClipboardHistory(ctx)
	must(t, err)
	var out []string
	for _, e := range history {
		out = append(out, e.Title)
	}
	return out
}

func equal[T any](t *testing.T, what string, got, want T) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Errorf("%s: %v, want %v", what, got, want)
	}
}

func TestUsageAndPinsRoundTrip(t *testing.T) {
	s := memory(t)
	must(t, s.RecordUse(ctx, "app:/a", core.Use{Count: 3, LastUsed: 9}))
	// An older use that saves late never moves the row back.
	must(t, s.RecordUse(ctx, "app:/a", core.Use{Count: 2, LastUsed: 8}))
	usage, err := s.Usage(ctx)
	must(t, err)
	equal(t, "usage", usage, map[core.ID]core.Use{"app:/a": {Count: 3, LastUsed: 9}})

	must(t, s.SetPinned(ctx, "b", true))
	must(t, s.SetPinned(ctx, "a", true))
	must(t, s.SetPinned(ctx, "b", true))
	pins, err := s.Pins(ctx)
	must(t, err)
	equal(t, "pins", pins, []core.ID{"b", "a"})
	must(t, s.SetPinned(ctx, "b", false))
	pins, err = s.Pins(ctx)
	must(t, err)
	equal(t, "pins", pins, []core.ID{"a"})
}

func TestUsageKeepsTheMostRecentlyUsedRows(t *testing.T) {
	s := memory(t)
	for n := range maxUsage + 1 {
		must(t, s.RecordUse(ctx, core.ID(fmt.Sprintf("app:/%04d", n)), core.Use{Count: 1, LastUsed: int64(n)}))
	}
	usage, err := s.Usage(ctx)
	must(t, err)
	if _, kept := usage["app:/0000"]; kept || len(usage) != maxUsage {
		t.Errorf("%d rows, oldest kept: %v", len(usage), kept)
	}
}

func TestRepeatsMoveToTheTopAndPinsSurvivePruning(t *testing.T) {
	s := memory(t)
	for i, value := range []string{"one", "two", "three"} {
		must(t, s.SaveClip(ctx, clipboard.Text(value), int64(i+1), 10))
	}
	must(t, s.SaveClip(ctx, clipboard.Text("one"), 4, 10))
	equal(t, "titles", titles(t, s), []string{"one", "three", "two"})

	history, err := s.ClipboardHistory(ctx)
	must(t, err)
	two := history[2].ID
	must(t, s.SetPinned(ctx, clipID(two), true))
	must(t, s.SaveClip(ctx, clipboard.Text("four"), 5, 2))
	equal(t, "titles", titles(t, s), []string{"four", "one", "two"})

	must(t, s.SaveClip(ctx, clipboard.Text("five"), 6, 10))
	must(t, s.TrimClipboard(ctx, 1))
	equal(t, "titles", titles(t, s), []string{"five", "two"})

	must(t, s.ClearClipboard(ctx))
	equal(t, "titles", titles(t, s), []string{"two"})
	must(t, s.DeleteClip(ctx, two))
	equal(t, "titles", titles(t, s), nil)
	pins, err := s.Pins(ctx)
	must(t, err)
	equal(t, "pins", pins, nil)
}

func TestARepeatInTheSameSecondMovesToTheTop(t *testing.T) {
	s := memory(t)
	for _, value := range []string{"one", "two", "one"} {
		must(t, s.SaveClip(ctx, clipboard.Text(value), 1, 10))
	}
	equal(t, "titles", titles(t, s), []string{"one", "two"})
	must(t, s.SaveClip(ctx, clipboard.Text("three"), 1, 10))
	equal(t, "titles", titles(t, s), []string{"three", "one", "two"})
	must(t, s.TrimClipboard(ctx, 2))
	equal(t, "titles", titles(t, s), []string{"three", "one"})
}

func TestStoresImagesAndFilesInTheirOriginalForm(t *testing.T) {
	s := memory(t)
	image := clipboard.Image([]byte{1, 2, 3}, 2, 1)
	files := clipboard.Files([]string{"/tmp/a b.txt", "/tmp/<&> .txt"})
	empty := clipboard.Image([]byte{}, 1, 1)
	must(t, s.SaveClip(ctx, image, 1, 10))
	must(t, s.SaveClip(ctx, files, 2, 10))
	must(t, s.SaveClip(ctx, empty, 3, 10))
	history, err := s.ClipboardHistory(ctx)
	must(t, err)
	equal(t, "searchable paths", history[1].Text, "/tmp/a b.txt\n/tmp/<&> .txt")
	for i, want := range []clipboard.Content{empty, files, image} {
		got, ok, err := s.Clip(ctx, history[i].ID)
		must(t, err)
		if !ok || !reflect.DeepEqual(got, want) {
			t.Errorf("entry %d: %+v, want %+v", i, got, want)
		}
	}
	if _, ok, err := s.Clip(ctx, 999); ok || err != nil {
		t.Errorf("a missing entry: %v %v", ok, err)
	}
	if err := s.SaveClip(ctx, clipboard.Files([]string{"/tmp/\xff"}), 4, 10); err == nil {
		t.Error("a path that is not UTF-8 was saved")
	}
}

func TestImagesBeyondTheCapArePruned(t *testing.T) {
	s := memory(t)
	for n := range clipboard.MaxImages + 2 {
		must(t, s.SaveClip(ctx, clipboard.Image([]byte(strconv.Itoa(n)), 1, 1), int64(n), 1000))
	}
	history, err := s.ClipboardHistory(ctx)
	must(t, err)
	equal(t, "entries", len(history), clipboard.MaxImages)
}

func TestLibraryItemsSaveUpdateAndDelete(t *testing.T) {
	s := memory(t)
	saved, err := s.SaveLibraryItem(ctx, library.Item{Kind: library.KindSnippet, Name: "Sig", Keyword: "sig", Text: "Regards"})
	must(t, err)
	key := strconv.FormatInt(saved.ID, 10)
	must(t, s.SetPinned(ctx, core.SourceSnippet.ID(key), true))
	must(t, s.SetHidden(ctx, core.SourceSnippet.ID(key), true, 1))
	must(t, s.SetHidden(ctx, core.SourceLink.ID(key), true, 1))
	must(t, s.SetHidden(ctx, "app:/a", true, 1))
	must(t, s.RecordUse(ctx, core.SourceSnippet.ID(key), core.Use{Count: 1, LastUsed: 1}))
	changed := saved
	changed.Text = "Cheers"
	_, err = s.SaveLibraryItem(ctx, changed)
	must(t, err)
	items, err := s.Library(ctx)
	must(t, err)
	equal(t, "items", items, []library.Item{changed})

	must(t, s.DeleteLibraryItem(ctx, saved.ID))
	items, err = s.Library(ctx)
	must(t, err)
	equal(t, "items", items, nil)
	pins, err := s.Pins(ctx)
	must(t, err)
	equal(t, "pins", pins, nil)
	hidden, err := s.Hidden(ctx)
	must(t, err)
	equal(t, "hidden", hidden, []core.ID{"app:/a"})
	usage, err := s.Usage(ctx)
	must(t, err)
	equal(t, "usage", len(usage), 0)

	if _, err := s.SaveLibraryItem(ctx, saved); err == nil || !strings.Contains(err.Error(), "was deleted") {
		t.Errorf("updating a deleted item: %v", err)
	}
	items, err = s.Library(ctx)
	must(t, err)
	equal(t, "items", items, nil)
}

func TestHidesAndShowsResultsAgain(t *testing.T) {
	s := memory(t)
	must(t, s.SetHidden(ctx, "app:/b", true, 2))
	must(t, s.SetHidden(ctx, "app:/a", true, 1))
	must(t, s.SetHidden(ctx, "app:/a", true, 3))
	hidden, err := s.Hidden(ctx)
	must(t, err)
	equal(t, "hidden", hidden, []core.ID{"app:/a", "app:/b"})
	must(t, s.SetHidden(ctx, "app:/a", false, 4))
	hidden, err = s.Hidden(ctx)
	must(t, err)
	equal(t, "hidden", hidden, []core.ID{"app:/b"})
}

func TestHotkeysAndAliasesSaveReplaceAndGoWithTheirItem(t *testing.T) {
	type list struct {
		name   string
		get    func(context.Context) (map[core.ID]string, error)
		set    func(context.Context, core.ID, string) error
		remove func(context.Context, core.ID) error
	}
	s := memory(t)
	for _, l := range []list{
		{"hotkeys", s.Hotkeys, s.SetHotkey, s.RemoveHotkey},
		{"aliases", s.Aliases, s.SetAlias, s.RemoveAlias},
	} {
		must(t, l.set(ctx, "app:/a", "A"))
		must(t, l.set(ctx, "app:/a", "B"))
		if err := l.set(ctx, "app:/b", "B"); err == nil {
			t.Errorf("%s: two results share one", l.name)
		}
		got, err := l.get(ctx)
		must(t, err)
		equal(t, l.name, got, map[core.ID]string{"app:/a": "B"})
		must(t, l.remove(ctx, "app:/a"))
		got, err = l.get(ctx)
		must(t, err)
		equal(t, l.name, len(got), 0)

		item, err := s.SaveLibraryItem(ctx, library.Item{Kind: library.KindSnippet, Name: "Sig", Text: "Regards"})
		must(t, err)
		must(t, l.set(ctx, core.SourceSnippet.ID(strconv.FormatInt(item.ID, 10)), "S"))
		must(t, s.DeleteLibraryItem(ctx, item.ID))
		got, err = l.get(ctx)
		must(t, err)
		equal(t, l.name, len(got), 0)
	}
}

func TestCalculationsMoveUpOnRepeatAndStayBounded(t *testing.T) {
	s := memory(t)
	expressions := func() []string {
		calculations, err := s.Calculations(ctx)
		must(t, err)
		var out []string
		for _, c := range calculations {
			out = append(out, c.Expression)
		}
		return out
	}
	must(t, s.SaveCalculation(ctx, "1+1", "2", 1))
	must(t, s.SaveCalculation(ctx, "2*3", "6", 2))
	must(t, s.SaveCalculation(ctx, "1+1", "2", 3))
	equal(t, "expressions", expressions(), []string{"1+1", "2*3"})
	// A repeat within the same second still moves to the top.
	must(t, s.SaveCalculation(ctx, "2*3", "6", 3))
	must(t, s.SaveCalculation(ctx, "1+1", "2", 3))
	equal(t, "expressions", expressions(), []string{"1+1", "2*3"})
	for n := range maxCalculations {
		must(t, s.SaveCalculation(ctx, fmt.Sprintf("%d+0", n), strconv.Itoa(n), int64(10+n)))
	}
	kept := expressions()
	if len(kept) != maxCalculations || slices.Contains(kept, "1+1") {
		t.Errorf("%d kept, oldest among them: %v", len(kept), slices.Contains(kept, "1+1"))
	}
	calculations, err := s.Calculations(ctx)
	must(t, err)
	must(t, s.DeleteCalculation(ctx, calculations[0].ID))
	equal(t, "count", len(expressions()), maxCalculations-1)
	must(t, s.ClearCalculations(ctx))
	equal(t, "expressions", expressions(), nil)
}

func TestRatesRoundTrip(t *testing.T) {
	s := memory(t)
	if _, ok, err := s.Rates(ctx); ok || err != nil {
		t.Errorf("rates before any: %v %v", ok, err)
	}
	must(t, s.SaveRates(ctx, `{"rates":{}}`))
	must(t, s.SaveRates(ctx, `{"rates":{"USD":1.16}}`))
	rates, ok, err := s.Rates(ctx)
	if !ok || err != nil || rates != `{"rates":{"USD":1.16}}` {
		t.Errorf("%q %v %v", rates, ok, err)
	}
	must(t, s.DeleteRates(ctx))
	if _, ok, _ := s.Rates(ctx); ok {
		t.Error("rates after delete")
	}
}

func TestOpeningMakesTheFilePrivateOrSaysSo(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "new", "folder")
	var restricted string
	s, warning, err := Open(ctx, dir, func(path string) error { restricted = path; return nil })
	must(t, err)
	must(t, s.Close())
	if warning != "" || restricted != filepath.Join(dir, FileName) {
		t.Errorf("warning %q, restricted %q", warning, restricted)
	}
	s, warning, err = Open(ctx, dir, func(string) error { return errors.New("no ACLs") })
	must(t, err)
	must(t, s.Close())
	if !strings.Contains(warning, "(no ACLs)") || !strings.Contains(warning, "readable only by you") {
		t.Errorf("warning %q", warning)
	}
}

func TestADamagedDatabaseSaysToMoveItAsideNotToFreeSpace(t *testing.T) {
	dir := t.TempDir()
	must(t, os.WriteFile(filepath.Join(dir, FileName), []byte(strings.Repeat("x", 4096)), 0o600))
	_, _, err := Open(ctx, dir, func(string) error { return nil })
	if err == nil || !strings.Contains(err.Error(), "move tinydash.db out of its data folder") || strings.Contains(err.Error(), "disk") {
		t.Errorf("%v", err)
	}
	var data *DataError
	if !errors.As(err, &data) {
		t.Errorf("%T is no DataError", err)
	}
}

func TestADataFolderThatCannotBeMadeFails(t *testing.T) {
	file := filepath.Join(t.TempDir(), "file")
	must(t, os.WriteFile(file, nil, 0o600))
	_, _, err := Open(ctx, filepath.Join(file, "dir"), func(string) error { return nil })
	if err == nil || !strings.HasPrefix(err.Error(), "Could not read or write a file: ") {
		t.Errorf("%v", err)
	}
}

func rawDB(t *testing.T, path string) *sqlite.DB {
	t.Helper()
	db, err := sqlite.Open(ctx, path, sqlite.OpenOptions{})
	must(t, err)
	return db
}

func TestAnOlderDatabaseUpgradesAndKeepsItsData(t *testing.T) {
	dir := t.TempDir()
	db := rawDB(t, filepath.Join(dir, FileName))
	var batch []sqlite.Statement
	for _, sql := range statements(migrations[0]) {
		batch = append(batch, sqlite.Statement{SQL: sql})
	}
	batch = append(batch,
		sqlite.Statement{SQL: "PRAGMA user_version = 1"},
		sqlite.Statement{SQL: "INSERT INTO pins (id, position) VALUES ('app:/a', 0)"})
	_, err := db.Transaction(ctx, batch)
	must(t, err)
	must(t, db.Close())

	s := openDir(t, dir)
	version, err := s.db.Query(ctx, "PRAGMA user_version")
	must(t, err)
	equal(t, "user_version", version.Values[0][0], any(int64(len(migrations))))
	pins, err := s.Pins(ctx)
	must(t, err)
	equal(t, "pins", pins, []core.ID{"app:/a"})
	must(t, s.SetAlias(ctx, "app:/a", "aa"))
	must(t, s.SaveCalculation(ctx, "1+1", "2", 1))
	secure, err := s.db.Query(ctx, "PRAGMA secure_delete")
	must(t, err)
	equal(t, "secure_delete", secure.Values[0][0], any(int64(1)))
}

func TestNewerSchemasAreRefused(t *testing.T) {
	for _, version := range []int{len(migrations) + 1, -1} {
		dir := t.TempDir()
		db := rawDB(t, filepath.Join(dir, FileName))
		_, err := db.Execute(ctx, fmt.Sprintf("PRAGMA user_version = %d", version))
		must(t, err)
		must(t, db.Close())
		_, _, err = Open(ctx, dir, func(string) error { return nil })
		if err == nil || err.Error() != "The database was created by a newer TinyDash. Update TinyDash to use it." {
			t.Errorf("version %d: %v", version, err)
		}
	}
}

func TestStatementsSplitOutsideQuotesAndComments(t *testing.T) {
	cases := []struct {
		sql  string
		want []string
	}{
		{"\n  CREATE TABLE a (x);\n  CREATE TABLE b (y);\n", []string{"\n  CREATE TABLE a (x);", "\n  CREATE TABLE b (y);"}},
		{"SELECT 'a;b''c;';", []string{"SELECT 'a;b''c;';"}},
		{`SELECT "x;" FROM [t;];`, []string{`SELECT "x;" FROM [t;];`}},
		{"-- a; comment\nSELECT 1 /* ; */;\n-- trailing;\n", []string{"-- a; comment\nSELECT 1 /* ; */;"}},
		{"SELECT 1", []string{"SELECT 1"}},
		{" ; ;\n", nil},
	}
	for _, c := range cases {
		equal(t, fmt.Sprintf("%q", c.sql), statements(c.sql), c.want)
	}
	for i, m := range migrations {
		if got := strings.Join(statements(m), ""); strings.TrimSpace(got) != strings.TrimSpace(m) {
			t.Errorf("migration %d loses text when split", i+1)
		}
	}
}
