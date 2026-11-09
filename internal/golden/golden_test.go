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
