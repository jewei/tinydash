package system

import (
	"cmp"
	"encoding/json"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/match"
)

func titles(query string) []string {
	hits := Search(match.New(query), Context{})
	slices.SortStableFunc(hits, func(a, b core.Scored) int { return cmp.Compare(b.Score, a.Score) })
	var out []string
	for _, hit := range hits {
		out = append(out, hit.Result.Title)
	}
	return out
}

func TestAliasesFindCommands(t *testing.T) {
	for query, want := range map[string]string{
		"reboot": "Restart", "suspend": "Sleep", "sign out": "Log Out", "dark mode": "Toggle Dark Mode",
		"remove formatting": "Make Clipboard Plain Text", "screen off": sleepDisplays,
	} {
		if got := titles(query); len(got) == 0 || got[0] != want {
			t.Errorf("%q: %q, want %q first", query, got, want)
		}
	}
}

func TestDestructiveCommandsAskFirst(t *testing.T) {
	for key, asks := range map[string]bool{"shut-down": true, "empty-trash": true, "lock": false} {
		r, ok := Get(key, Context{})
		if !ok || (r.Actions[0].Confirm != "") != asks {
			t.Errorf("%s: %+v", key, r.Actions)
		}
	}
	if _, ok := Get("nothing", Context{}); ok {
		t.Error("found a missing command")
	}
}

func TestOnlyRestartsThatAskAppsToQuitSaySo(t *testing.T) {
	// Linux falls back to systemd, which does not ask apps.
	asks := !strings.Contains(confirmRestart, "Save your work")
	for _, key := range []string{"restart", "shut-down"} {
		r, _ := Get(key, Context{})
		if strings.Contains(r.Actions[0].Confirm, "asked to quit") != asks {
			t.Errorf("%s: %q", key, r.Actions[0].Confirm)
		}
	}
}

func TestEveryCommandHasAUniqueKeyAndEveryOSCommandIsListed(t *testing.T) {
	keys := map[string]bool{}
	listed := map[core.SystemCommand]bool{}
	for _, c := range commands {
		if keys[c.key] {
			t.Errorf("two commands have the key %q", c.key)
		}
		keys[c.key] = true
		if s, ok := c.action.(core.System); ok {
			listed[s.Command] = true
		}
	}
	for _, c := range core.SystemCommands() {
		if !listed[c] {
			t.Errorf("%s has no row", c)
		}
	}
}

// The System tab of testdata/golden/search.jsonl, without the actions that
// search adds (alias, hotkey, hide). It ran on macOS; other OSes have their
// own labels.
func TestBrowseMatchesRust(t *testing.T) {
	if emptyTrash != "Empty Trash" || sleepDisplays != "Sleep Displays" || strings.Contains(confirmRestart, "Save") {
		t.Skip("the corpus has the macOS labels")
	}
	type row struct {
		Category string            `json:"category"`
		Query    string            `json:"query"`
		Results  []json.RawMessage `json:"results"`
	}
	for _, r := range golden.Rows[row](t, "search") {
		if r.Category != "system" || r.Query != "" {
			continue
		}
		got := Browse(Context{})
		if len(got) != len(r.Results) {
			t.Fatalf("%d results, Rust had %d", len(got), len(r.Results))
		}
		for i, want := range r.Results {
			var rust map[string]any
			if err := json.Unmarshal(want, &rust); err != nil {
				t.Fatal(err)
			}
			actions := rust["actions"].([]any)
			rust["actions"] = actions[:2]
			data, _ := json.Marshal(rust)
			golden.Check(t, "system", string(got[i].ID), data, golden.Result(got[i]))
		}
		return
	}
	t.Fatal("no empty System search in the corpus")
}
