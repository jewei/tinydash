// Package golden compares the Go port with what the Rust app answered, as
// recorded in testdata/golden. Tests use it; the app does not.
package golden

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

// Dir is testdata/golden at the module root.
func Dir(t testing.TB) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return filepath.Join(dir, "testdata", "golden")
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("no go.mod above the test")
		}
		dir = parent
	}
}

// Rows decodes each line of <feature>.jsonl into T.
func Rows[T any](t testing.TB, feature string) []T {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(Dir(t), feature+".jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	var rows []T
	scanner := bufio.NewScanner(bytes.NewReader(data))
	scanner.Buffer(nil, 1<<24)
	for scanner.Scan() {
		var row T
		if err := json.Unmarshal(scanner.Bytes(), &row); err != nil {
			t.Fatalf("%s.jsonl: %v", feature, err)
		}
		rows = append(rows, row)
	}
	return rows
}

// Meta is the fixed world the corpus was written in.
type Meta struct {
	TZ    string `json:"tz"`
	Clock string `json:"now"`
	Rates struct {
		Rates map[string]float64 `json:"rates"`
	} `json:"rates"`
}

// Now is the corpus clock, in its time zone.
func (m Meta) Now(t testing.TB) time.Time {
	t.Helper()
	loc, err := time.LoadLocation(m.TZ)
	if err != nil {
		t.Fatal(err)
	}
	now, err := time.Parse(time.RFC3339, m.Clock)
	if err != nil {
		t.Fatal(err)
	}
	return now.In(loc)
}

// LoadMeta reads meta.jsonl.
func LoadMeta(t testing.TB) Meta { return Rows[Meta](t, "meta")[0] }

// Result renders a result as the Rust app serialized it.
func Result(r core.Result) map[string]any {
	actions := make([]any, len(r.Actions))
	for i, a := range r.Actions {
		var confirm any
		if a.Confirm != "" {
			confirm = a.Confirm
		}
		actions[i] = map[string]any{"label": a.Label, "action": Action(a.Action), "confirm": confirm}
	}
	return map[string]any{
		"id": string(r.ID), "kind": string(r.Kind), "title": r.Title, "subtitle": r.Subtitle,
		"icon": icon(r.Icon), "actions": actions, "pinned": r.Pinned,
	}
}

// Action renders an action as {"type": name, fields...}.
func Action(a core.Action) map[string]any {
	out := map[string]any{}
	data, err := json.Marshal(a)
	if err != nil {
		panic(err)
	}
	if err := json.Unmarshal(data, &out); err != nil {
		panic(err)
	}
	out["type"] = a.Name()
	return out
}

func icon(i core.Icon) map[string]any {
	switch {
	case i.File != "":
		return map[string]any{"type": "file", "path": i.File}
	case i.Emoji != "":
		return map[string]any{"type": "emoji", "glyph": i.Emoji}
	case i.Color != "":
		return map[string]any{"type": "color", "hex": i.Color}
	default:
		return map[string]any{"type": "symbol", "name": string(i.Symbol)}
	}
}

// Results renders a list of results.
func Results(rs []core.Result) []any {
	out := make([]any, len(rs))
	for i, r := range rs {
		out[i] = Result(r)
	}
	return out
}

var (
	divergencesOnce sync.Once
	divergences     map[[2]string]string
)

func loadDivergences(t testing.TB) map[[2]string]string {
	divergencesOnce.Do(func() {
		divergences = map[[2]string]string{}
		files, err := filepath.Glob(filepath.Join(Dir(t), "divergences", "*.tsv"))
		if err != nil {
			t.Fatal(err)
		}
		for _, file := range files {
			data, err := os.ReadFile(file)
			if err != nil {
				t.Fatal(err)
			}
			feature := strings.TrimSuffix(filepath.Base(file), ".tsv")
			for i, line := range strings.Split(strings.TrimRight(string(data), "\n"), "\n") {
				if i == 0 || line == "" {
					continue // header: query, reason
				}
				query, reason, ok := strings.Cut(line, "\t")
				if !ok || strings.TrimSpace(reason) == "" {
					t.Fatalf("%s line %d: want query, tab, reason", file, i+1)
				}
				divergences[[2]string{feature, query}] = reason
			}
		}
	})
	return divergences
}

// Check compares what the port gives for a query with what Rust gave. A
// difference fails unless divergences/<feature>.tsv lists it; a listed query that
// now matches fails too, so the list stays true.
func Check(t testing.TB, feature, query string, want json.RawMessage, got any) {
	t.Helper()
	var wantValue, gotValue any
	if err := json.Unmarshal(want, &wantValue); err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(got)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, &gotValue); err != nil {
		t.Fatal(err)
	}
	_, listed := loadDivergences(t)[[2]string{feature, query}]
	same := reflect.DeepEqual(wantValue, gotValue)
	switch {
	case !same && !listed:
		t.Errorf("%s %q:\n Rust: %s\n Go:   %s", feature, query, want, data)
	case same && listed:
		t.Errorf("%s %q matches Rust now; remove it from divergences/%s.tsv", feature, query, feature)
	}
}

// Quote formats a query for messages.
func Quote(s string) string { return fmt.Sprintf("%q", s) }

// Ranked is one entry of a ranked list, with the score that placed it.
type Ranked struct {
	ID    string `json:"id"`
	Score uint32 `json:"score"`
}

// CheckRanked compares a ranked list with Rust's, where only the order
// within equal scores may differ: the order of ties in Rust came from an
// unstable selection and is not behavior. The scores must match in order,
// every Rust entry must score the same in Go (scoreOf), and each group of
// equal scores must hold the same entries, except the last group, which a
// limit may have cut.
func CheckRanked(t testing.TB, feature, query string, want, got []Ranked, scoreOf func(id string) (uint32, bool)) {
	t.Helper()
	fail := func(format string, args ...any) {
		t.Errorf("%s %q: "+format+"\n Rust: %v\n Go:   %v", append(append([]any{feature, query}, args...), want, got)...)
	}
	if len(want) != len(got) {
		fail("%d entries, Rust had %d", len(got), len(want))
		return
	}
	for i := range want {
		if want[i].Score != got[i].Score {
			fail("score %d at %d, Rust had %d", got[i].Score, i, want[i].Score)
			return
		}
		if score, ok := scoreOf(want[i].ID); !ok || score != want[i].Score {
			fail("%s scores %d (found %v) in Go, %d in Rust", want[i].ID, score, ok, want[i].Score)
			return
		}
	}
	for start := 0; start < len(want); {
		end := start
		for end < len(want) && want[end].Score == want[start].Score {
			end++
		}
		if end == len(want) {
			break // the last group: the limit may have cut it anywhere
		}
		wantIDs, gotIDs := map[string]bool{}, map[string]bool{}
		for i := start; i < end; i++ {
			wantIDs[want[i].ID], gotIDs[got[i].ID] = true, true
		}
		if !reflect.DeepEqual(wantIDs, gotIDs) {
			fail("entries scoring %d differ", want[start].Score)
			return
		}
		start = end
	}
}
