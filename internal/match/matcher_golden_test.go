package match

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/golden"
)

// optional renders a score as Rust serialized an Option<u32>.
func optional(score uint32, ok bool) any {
	if !ok {
		return nil
	}
	return score
}

// Every query against every candidate, and each query normalized, as the
// Rust app scored them.
func TestMatcherGolden(t *testing.T) {
	rows := golden.Rows[json.RawMessage](t, "matcher")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	for _, raw := range rows {
		var row struct {
			Query     *string `json:"query"`
			Candidate string  `json:"candidate"`
			Normalize *string `json:"normalize"`
		}
		if err := json.Unmarshal(raw, &row); err != nil {
			t.Fatal(err)
		}
		switch {
		case row.Normalize != nil:
			got := map[string]any{"normalize": *row.Normalize, "normalized": Normalize(*row.Normalize)}
			golden.Check(t, "matcher", "normalize "+*row.Normalize, raw, got)
		case row.Query != nil:
			m := New(*row.Query)
			got := map[string]any{
				"query":     *row.Query,
				"candidate": row.Candidate,
				"name":      optional(m.Name(row.Candidate)),
				"words":     optional(m.Words(row.Candidate)),
				"alias":     optional(m.Alias(row.Candidate)),
			}
			golden.Check(t, "matcher", *row.Query+" ~ "+row.Candidate, raw, got)
		default:
			t.Fatalf("row of neither shape: %s", raw)
		}
	}
}
