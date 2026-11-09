package emoji

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/match"
)

// The Rust app's top eight per query, with English names and shortcodes
// only, no usage, and no pins.
func TestEmojiGolden(t *testing.T) {
	rows := golden.Rows[struct {
		Query string          `json:"query"`
		Top   json.RawMessage `json:"top"`
	}](t, "emoji")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	index := NewIndex()
	for _, row := range rows {
		hits := index.Search(match.New(row.Query), Context{}, 8)
		top := make([]map[string]any, len(hits))
		for i, hit := range hits {
			top[i] = map[string]any{"id": string(hit.Result.ID), "score": hit.Score}
		}
		golden.Check(t, "emoji", row.Query, row.Top, top)
	}
}
