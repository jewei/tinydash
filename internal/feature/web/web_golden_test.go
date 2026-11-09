package web

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/golden"
)

func TestGolden(t *testing.T) {
	rows := golden.Rows[struct {
		Query  string          `json:"query"`
		Answer json.RawMessage `json:"answer"`
		Encode json.RawMessage `json:"encode"`
	}](t, "web")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	for _, row := range rows {
		var got any
		if r, ok := Answer(row.Query); ok {
			got = golden.Result(r)
		}
		golden.Check(t, "web", row.Query, row.Answer, got)
		golden.Check(t, "web", row.Query, row.Encode, Encode(row.Query))
	}
}
