package store

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"strconv"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/golden"
)

// Shipped migrations, by SHA-256. A changed entry would leave databases
// that ran the old one with a different schema than new ones.
var shipped = []string{
	"403dbbcfe803ac3c958fead61799297cd198e620e3c9905ae04381140d262ea7",
	"f0f91762812245548ba47428ca822c852a3c224aae097c344089c58081f45c0d",
	"4fa30311180aa7fdd0ac416375c8d425912ec4ac2346b19b19f61878fbc8df6b",
	"a89f1a30795f3f2ebcfa4df96b0711aef3511a5ef99b40dfa308c91abffc2abc",
	"efe00a69d1201a54b3684440e0b6addec381bb1fff71dc62d4e867520a5f1dcc",
	"5ad8350c67ac88d9432e13e95c9136acacd5e11b9a110ef425b7fe3353f550be",
}

func TestMigrationsNeverChange(t *testing.T) {
	if len(migrations) < len(shipped) {
		t.Fatalf("%d migrations, %d shipped", len(migrations), len(shipped))
	}
	for i, want := range shipped {
		sum := sha256.Sum256([]byte(migrations[i]))
		if got := hex.EncodeToString(sum[:]); got != want {
			t.Errorf("migration %d has SHA-256 %s, shipped %s; append a migration instead", i+1, got, want)
		}
	}
}

// fixture is store.jsonl: every table of tinydash.db as the Rust app left
// it, and the hash and title of sample contents.
type fixture struct {
	UserVersion json.Number                 `json:"user_version"`
	Tables      map[string][]map[string]any `json:"tables"`
	Hashes      []struct {
		Kind  string      `json:"kind"`
		Hash  json.Number `json:"hash"`
		Title string      `json:"title"`
	} `json:"hashes"`
}

func loadFixture(t *testing.T) fixture {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(golden.Dir(t), "fixtures", "store.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	var f fixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	if err := decoder.Decode(&f); err != nil {
		t.Fatal(err)
	}
	return f
}

// samples are the contents whose hashes store.jsonl lists, in its order.
var samples = []clipboard.Content{
	clipboard.Text("hello world"),
	clipboard.Text("東京 🚀"),
	clipboard.Files([]string{"/tmp/a.txt", "/tmp/b b.png"}),
	clipboard.Image([]byte{137, 80, 78, 71, 1, 2, 3}, 2, 1),
}

func TestClipboardHashesMatchRust(t *testing.T) {
	f := loadFixture(t)
	if len(f.Hashes) != len(samples) {
		t.Fatalf("%d hashes, %d samples", len(f.Hashes), len(samples))
	}
	kinds := map[string]clipboard.Kind{"Text": clipboard.KindText, "Image": clipboard.KindImage, "Files": clipboard.KindFiles}
	for i, want := range f.Hashes {
		c := samples[i]
		if got := strconv.FormatInt(c.Hash(), 10); got != want.Hash.String() || c.Title() != want.Title || c.Kind != kinds[want.Kind] {
			t.Errorf("sample %d: %s %s %q, Rust %s %s %q", i, c.Kind, got, c.Title(), want.Kind, want.Hash, want.Title)
		}
	}
}

func openDir(t *testing.T, dir string) *Store {
	t.Helper()
	s, warning, err := Open(context.Background(), dir, func(string) error { return nil })
	if err != nil || warning != "" {
		t.Fatalf("open: %v %q", err, warning)
	}
	t.Cleanup(func() {
		if err := s.Close(); err != nil {
			t.Error(err)
		}
	})
	return s
}

// The Go store reads the database the Rust app wrote, and opening it
// changes nothing.
func TestReadsTheRustDatabase(t *testing.T) {
	ctx := context.Background()
	f := loadFixture(t)
	dir := t.TempDir()
	data, err := os.ReadFile(filepath.Join(golden.Dir(t), "fixtures", FileName))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, FileName), data, 0o600); err != nil {
		t.Fatal(err)
	}
	s := openDir(t, dir)
	tables := f.Tables
	rows := func(table string) []map[string]any { return tables[table] }
	str := func(v any) string { return v.(string) }
	num := func(v any) int64 {
		n, err := v.(json.Number).Int64()
		if err != nil {
			t.Fatal(err)
		}
		return n
	}

	usage, err := s.Usage(ctx)
	wantUsage := map[core.ID]core.Use{}
	for _, r := range rows("usage") {
		wantUsage[core.ID(str(r["id"]))] = core.Use{Count: uint32(num(r["count"])), LastUsed: num(r["last_used"])}
	}
	check(t, "usage", usage, wantUsage, err)

	pins, err := s.Pins(ctx)
	byPosition := slices.Clone(rows("pins"))
	slices.SortFunc(byPosition, func(a, b map[string]any) int { return int(num(a["position"]) - num(b["position"])) })
	var wantPins []core.ID
	for _, r := range byPosition {
		wantPins = append(wantPins, core.ID(str(r["id"])))
	}
	check(t, "pins", pins, wantPins, err)

	history, err := s.ClipboardHistory(ctx)
	newest := slices.Clone(rows("clipboard"))
	slices.SortFunc(newest, func(a, b map[string]any) int {
		if d := num(b["copied_at"]) - num(a["copied_at"]); d != 0 {
			return int(d)
		}
		return int(num(b["id"]) - num(a["id"]))
	})
	var wantHistory []clipboard.Entry
	for _, r := range newest {
		e := clipboard.Entry{ID: num(r["id"]), Kind: clipboard.Kind(str(r["kind"])), Title: str(r["title"]), Text: str(r["content"]), CopiedAt: num(r["copied_at"])}
		if e.Kind == clipboard.KindFiles {
			e.Text = "/tmp/a.txt\n/tmp/b b.png"
		}
		wantHistory = append(wantHistory, e)

		content, ok, err := s.Clip(ctx, e.ID)
		var want clipboard.Content
		switch e.Kind {
		case clipboard.KindText:
			want = clipboard.Text(str(r["content"]))
		case clipboard.KindFiles:
			want = clipboard.Files([]string{"/tmp/a.txt", "/tmp/b b.png"})
		case clipboard.KindImage:
			png, _ := hex.DecodeString(str(r["image"].(map[string]any)["hex"]))
			want = clipboard.Image(png, uint32(num(r["width"])), uint32(num(r["height"])))
		}
		check(t, fmt.Sprintf("clip %d", e.ID), content, want, err)
		if !ok || content.Hash() != num(r["hash"]) {
			t.Errorf("clip %d: found %v, hash %d, stored %d", e.ID, ok, content.Hash(), num(r["hash"]))
		}
	}
	check(t, "clipboard history", history, wantHistory, err)

	items, err := s.Library(ctx)
	var wantItems []library.Item
	for _, r := range rows("library") {
		wantItems = append(wantItems, library.Item{ID: num(r["id"]), Kind: library.Kind(str(r["kind"])), Name: str(r["name"]), Keyword: str(r["keyword"]), Text: str(r["text"])})
	}
	check(t, "library", items, wantItems, err)

	hidden, err := s.Hidden(ctx)
	check(t, "hidden", hidden, []core.ID{core.ID(str(rows("hidden")[0]["id"]))}, err)

	for table, read := range map[string]func(context.Context) (map[core.ID]string, error){"aliases": s.Aliases, "hotkeys": s.Hotkeys} {
		got, err := read(ctx)
		want := map[core.ID]string{}
		for _, r := range rows(table) {
			for column, v := range r {
				if column != "id" {
					want[core.ID(str(r["id"]))] = str(v)
				}
			}
		}
		check(t, table, got, want, err)
	}

	calculations, err := s.Calculations(ctx)
	var wantCalculations []Calculation
	for _, r := range slices.Backward(rows("calculations")) {
		wantCalculations = append(wantCalculations, Calculation{ID: num(r["id"]), Expression: str(r["expression"]), Result: str(r["result"])})
	}
	check(t, "calculations", calculations, wantCalculations, err)

	rates, ok, err := s.Rates(ctx)
	check(t, "rates", rates, str(rows("cache")[0]["value"]), err)
	if !ok {
		t.Error("no rates")
	}

	compareDump(t, s, f)
}

// The calls golden_fixtures (src-tauri/src/golden.rs) made, made again on
// a new database, write the same rows.
func TestReplaysTheRustCalls(t *testing.T) {
	ctx := context.Background()
	f := loadFixture(t)
	s := openDir(t, t.TempDir())
	const now = 1_794_191_400
	must := func(err error) {
		t.Helper()
		if err != nil {
			t.Fatal(err)
		}
	}
	must(s.RecordUse(ctx, "app:/Applications/Safari.app", core.Use{Count: 3, LastUsed: now - 86_400}))
	must(s.RecordUse(ctx, "emoji:🚀", core.Use{Count: 1, LastUsed: now}))
	must(s.SetPinned(ctx, "app:/Applications/Safari.app", true))
	must(s.SetPinned(ctx, "system:lock", true))
	must(s.SaveClip(ctx, clipboard.Text("hello world"), now-60, 100))
	must(s.SaveClip(ctx, clipboard.Text("second\nline"), now-30, 100))
	must(s.SaveClip(ctx, clipboard.Files([]string{"/tmp/a.txt", "/tmp/b b.png"}), now-20, 100))
	must(s.SaveClip(ctx, clipboard.Image([]byte{137, 80, 78, 71, 1, 2, 3}, 2, 1), now-10, 100))
	must(s.SaveClip(ctx, clipboard.Text("hello world"), now-10, 100))
	must(s.SetPinned(ctx, "clip:1", true))
	snippet, err := s.SaveLibraryItem(ctx, library.Item{Kind: library.KindSnippet, Name: "Signature", Keyword: "sig", Text: "Best,\n{clipboard}"})
	must(err)
	_, err = s.SaveLibraryItem(ctx, library.Item{Kind: library.KindQuicklink, Name: "Search MDN", Keyword: "mdn", Text: "https://developer.mozilla.org/search?q={query}"})
	must(err)
	must(s.SetHidden(ctx, "app:/Applications/Chess.app", true, now))
	must(s.SetAlias(ctx, "app:/Applications/Safari.app", "saf"))
	must(s.SetHotkey(ctx, core.SourceSnippet.ID(strconv.FormatInt(snippet.ID, 10)), "Control+Alt+S"))
	must(s.SaveCalculation(ctx, "12 * 8", "96", now-5))
	must(s.SaveCalculation(ctx, "5 ft to cm", "152.4 cm", now))
	must(s.SaveNote(ctx, "scratch note"))
	// The currency feature owns the JSON of its rates; this is the text
	// the Rust app wrote.
	must(s.SaveRates(ctx, f.Tables["cache"][0]["value"].(string)))

	compareDump(t, s, f)
}

// compareDump dumps every table as golden_fixtures did and compares it
// with store.jsonl, numbers exactly.
func compareDump(t *testing.T, s *Store, f fixture) {
	t.Helper()
	ctx := context.Background()
	version, err := s.db.Query(ctx, "PRAGMA user_version")
	if err != nil {
		t.Fatal(err)
	}
	if got := strconv.FormatInt(version.Values[0][0].(int64), 10); got != f.UserVersion.String() {
		t.Errorf("user_version %s, Rust %s", got, f.UserVersion)
	}
	names, err := s.db.Query(ctx, "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
	if err != nil {
		t.Fatal(err)
	}
	got := map[string][]map[string]any{}
	for _, name := range names.Values {
		table := name[0].(string)
		rows, err := s.db.Query(ctx, "SELECT * FROM "+table+" ORDER BY 1")
		if err != nil {
			t.Fatal(err)
		}
		dumped := []map[string]any{}
		for _, row := range rows.Values {
			object := map[string]any{}
			for i, column := range rows.Columns {
				switch v := row[i].(type) {
				case int64:
					object[column] = json.Number(strconv.FormatInt(v, 10))
				case []byte:
					object[column] = map[string]any{"hex": hex.EncodeToString(v)}
				default:
					object[column] = v
				}
			}
			dumped = append(dumped, object)
		}
		got[table] = dumped
	}
	for _, table := range sortedKeys(got, f.Tables) {
		if !reflect.DeepEqual(got[table], f.Tables[table]) {
			t.Errorf("table %s:\n Rust: %s\n Go:   %s", table, show(f.Tables[table]), show(got[table]))
		}
	}
}

func sortedKeys(a, b map[string][]map[string]any) []string {
	var keys []string
	for k := range a {
		keys = append(keys, k)
	}
	for k := range b {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	return slices.Compact(keys)
}

func show(v any) string {
	data, _ := json.Marshal(v)
	return string(data)
}

func check[T any](t *testing.T, what string, got, want T, err error) {
	t.Helper()
	if err != nil {
		t.Errorf("%s: %v", what, err)
	} else if !reflect.DeepEqual(got, want) {
		t.Errorf("%s:\n got  %+v\n want %+v", what, got, want)
	}
}
