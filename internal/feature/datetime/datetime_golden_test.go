package datetime

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/golden"
)

func TestGolden(t *testing.T) {
	type row struct {
		Query         string          `json:"query"`
		Answers       json.RawMessage `json:"answers"`
		IsDateOrClock json.RawMessage `json:"is_date_or_clock"`
	}
	now := golden.LoadMeta(t).Now(t)
	rows := golden.Rows[row](t, "datetime")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	for _, r := range rows {
		golden.Check(t, "datetime", r.Query, r.Answers, golden.Results(Answers(r.Query, now)))
		golden.Check(t, "datetime", r.Query, r.IsDateOrClock, IsDateOrClock(r.Query))
	}
}
