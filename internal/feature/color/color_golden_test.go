package color_test

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/feature/color"
	"github.com/jewei/tinydash/internal/feature/unixtime"
	"github.com/jewei/tinydash/internal/golden"
)

// The Rust app answered colors and Unix times in one function, color first;
// its corpus checks both packages.
func TestGolden(t *testing.T) {
	now := golden.LoadMeta(t).Now(t)
	rows := golden.Rows[struct {
		Query  string          `json:"query"`
		Answer json.RawMessage `json:"answer"`
	}](t, "clip_card")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	for _, row := range rows {
		var got any
		if r, ok := color.Answer(row.Query); ok {
			got = golden.Result(r)
		} else if r, ok := unixtime.Answer(row.Query, now); ok {
			got = golden.Result(r)
		}
		golden.Check(t, "clip_card", row.Query, row.Answer, got)
	}
}
