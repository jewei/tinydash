package clipcard_test

import (
	"encoding/json"
	"testing"

	"github.com/jewei/tinydash/internal/feature/clipcard"
	"github.com/jewei/tinydash/internal/golden"
)

// The Rust app's card and answer decoded a color or a Unix time the same
// way, with the same copy actions. Its corpus holds the answers, so a card
// must copy what the answer copied, and show no color or time where there
// was no answer.
func TestCardsCopyWhatTheRustAnswersCopied(t *testing.T) {
	now := golden.LoadMeta(t).Now(t)
	rows := golden.Rows[struct {
		Query  string `json:"query"`
		Answer *struct {
			Actions json.RawMessage `json:"actions"`
		} `json:"answer"`
	}](t, "clip_card")
	if len(rows) == 0 {
		t.Fatal("no golden rows")
	}
	for _, row := range rows {
		want := json.RawMessage("null")
		if row.Answer != nil {
			want = row.Answer.Actions
		}
		var got any
		if card, ok := clipcard.Detect(row.Query, now); ok && card.JSON == nil {
			actions := make([]any, len(card.Actions))
			for i, a := range card.Actions {
				actions[i] = map[string]any{"label": a.Label, "action": golden.Action(a.Action), "confirm": nil}
			}
			got = actions
		}
		golden.Check(t, "clip_card", row.Query, want, got)
	}
}
