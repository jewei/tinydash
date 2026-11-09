package cleanurl

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/golden"
)

func TestGolden(t *testing.T) {
	rows := golden.Rows[struct {
		Query  string          `json:"query"`
		Answer json.RawMessage `json:"answer"`
	}](t, "url_cleaner")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	for _, row := range rows {
		var got any
		if r, ok := Answer(row.Query); ok {
			got = golden.Result(r)
		}
		golden.Check(t, "url_cleaner", row.Query, row.Answer, got)
	}
}
