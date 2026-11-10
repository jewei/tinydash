package golden

import (
	"encoding/json"
	"fmt"
	"testing"
)

// spy records failures instead of failing the real test.
type spy struct {
	testing.TB
	errors []string
}

func (s *spy) Helper()                        {}
func (s *spy) Errorf(format string, a ...any) { s.errors = append(s.errors, fmt.Sprintf(format, a...)) }

func TestCheckFailsOnUnlistedDifferencesAndStaleListings(t *testing.T) {
	divergencesOnce.Do(func() {}) // use the table below, not the files
	divergences = map[[2]string]string{{"calc", "1 btc to usd"}: "no crypto rates"}
	want := json.RawMessage(`{"title":"96"}`)

	cases := []struct {
		name, query string
		got         any
		fails       bool
	}{
		{"same", "12 * 8", map[string]any{"title": "96"}, false},
		{"different and unlisted", "12 * 8", map[string]any{"title": "97"}, true},
		{"different and listed", "1 btc to usd", nil, false},
		{"listed but the same now", "1 btc to usd", map[string]any{"title": "96"}, true},
	}
	for _, c := range cases {
		s := &spy{TB: t}
		Check(s, "calc", c.query, want, c.got)
		if failed := len(s.errors) > 0; failed != c.fails {
			t.Errorf("%s: failed %v, want %v (%q)", c.name, failed, c.fails, s.errors)
		}
	}
}

func TestCheckRankedAllowsOnlyTieOrderToDiffer(t *testing.T) {
	r := func(id string, score uint32) Ranked { return Ranked{id, score} }
	want := []Ranked{r("a", 9), r("b", 5), r("c", 5), r("d", 2), r("e", 2)}
	scores := map[string]uint32{"a": 9, "b": 5, "c": 5, "d": 2, "e": 2, "f": 2}
	scoreOf := func(id string) (uint32, bool) { s, ok := scores[id]; return s, ok }
	cases := []struct {
		name  string
		got   []Ranked
		fails bool
	}{
		{"same", want, false},
		{"ties swapped", []Ranked{r("a", 9), r("c", 5), r("b", 5), r("e", 2), r("d", 2)}, false},
		{"last group cut elsewhere", []Ranked{r("a", 9), r("b", 5), r("c", 5), r("f", 2), r("d", 2)}, false},
		{"score differs", []Ranked{r("a", 9), r("b", 5), r("c", 4), r("d", 2), r("e", 2)}, true},
		{"a full tie group differs", []Ranked{r("a", 9), r("b", 5), r("f", 5), r("d", 2), r("e", 2)}, true},
		{"shorter", want[:4], true},
	}
	for _, c := range cases {
		s := &spy{TB: t}
		CheckRanked(s, "x", "q", want, c.got, scoreOf)
		if failed := len(s.errors) > 0; failed != c.fails {
			t.Errorf("%s: failed %v, want %v (%q)", c.name, failed, c.fails, s.errors)
		}
	}
	scores["e"] = 3 // Go scores a Rust entry differently
	s := &spy{TB: t}
	CheckRanked(s, "x", "q", want, want, scoreOf)
	if len(s.errors) == 0 {
		t.Error("a Rust entry with another score in Go should fail")
	}
}
