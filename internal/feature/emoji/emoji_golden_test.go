package emoji

import (
	"testing"

	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/match"
)

// The Rust app's top eight per query, with English names and shortcodes
// only, no usage, and no pins.
func TestEmojiGolden(t *testing.T) {
	rows := golden.Rows[struct {
		Query string          `json:"query"`
		Top   []golden.Ranked `json:"top"`
	}](t, "emoji")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	index := NewIndex()
	for _, row := range rows {
		hits := index.Search(match.New(row.Query), Context{}, 8)
		got := make([]golden.Ranked, len(hits))
		for i, hit := range hits {
			got[i] = golden.Ranked{ID: string(hit.Result.ID), Score: hit.Score}
		}
		all := map[string]uint32{}
		for _, hit := range index.Search(match.New(row.Query), Context{}, 100_000) {
			all[string(hit.Result.ID)] = hit.Score
		}
		golden.CheckRanked(t, "emoji", row.Query, row.Top, got, func(id string) (uint32, bool) {
			score, ok := all[id]
			return score, ok
		})
	}
}
