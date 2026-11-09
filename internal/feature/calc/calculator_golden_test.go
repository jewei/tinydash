package calc_test

import (
	"bytes"
	"encoding/json"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/calc"
	"github.com/jewei/tinydash/internal/feature/currency"
	"github.com/jewei/tinydash/internal/golden"
)

func TestCalculatorMatchesRust(t *testing.T) {
	meta := golden.Rows[struct {
		Rates json.RawMessage `json:"rates"`
	}](t, "meta")[0]
	rates, err := currency.Parse(bytes.NewReader(meta.Rates), time.Unix(0, 0))
	if err != nil {
		t.Fatal(err)
	}
	result := func(r core.Result, ok bool) any {
		if !ok {
			return nil
		}
		return golden.Result(r)
	}
	rows := golden.Rows[map[string]json.RawMessage](t, "calculator")
	for _, row := range rows {
		var query string
		if err := json.Unmarshal(row["query"], &query); err != nil {
			t.Fatal(err)
		}
		delete(row, "query")
		want, err := json.Marshal(row)
		if err != nil {
			t.Fatal(err)
		}
		expression := calc.Expression(query)
		golden.Check(t, "calculator", query, want, map[string]any{
			"expression":           expression,
			"answer":               result(calc.Answer(expression, rates)),
			"answer_without_rates": result(calc.Answer(expression, nil)),
			"rates_off":            result(calc.RatesOffAnswer(expression)),
		})
	}
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
}
