package search

import (
	"errors"
	"fmt"
	"slices"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func TestAliasesAreOneCleanWord(t *testing.T) {
	for _, c := range []struct {
		text, want string
		err        error
	}{
		{"  VSC ", "vsc", nil},
		{"", "", nil},
		{"two words", "", ErrAlias},
		{strings.Repeat("a", maxAlias+1), "", ErrAlias},
		{strings.Repeat("é", maxAlias), strings.Repeat("é", maxAlias), nil},
	} {
		got, err := CleanAlias(c.text)
		if got != c.want || !errors.Is(err, c.err) {
			t.Errorf("CleanAlias(%q) = %q, %v; want %q, %v", c.text, got, err, c.want, c.err)
		}
	}
	aliases := NewAliases(map[core.ID]string{"app:/Code.app": "vsc"})
	if owner, ok := aliases.Owner("vsc"); !ok || owner != "app:/Code.app" {
		t.Errorf("owner of vsc: %q %v", owner, ok)
	}
	changed := aliases.Set("app:/Code.app", "code")
	if _, ok := changed.Owner("vsc"); ok {
		t.Error("the old alias still has an owner")
	}
	if alias, _ := changed.Get("app:/Code.app"); alias != "code" {
		t.Errorf("alias %q, want code", alias)
	}
	if changed.Remove("app:/Code.app").Len() != 0 {
		t.Error("Remove kept the alias")
	}
	if alias, _ := aliases.Get("app:/Code.app"); alias != "vsc" {
		t.Errorf("Set changed the old value: %q", alias)
	}
}

func TestFrequencyAndRecencyAreBounded(t *testing.T) {
	now := int64(day * 100)
	usage := NewUsage(nil)
	for range 100 {
		usage, _ = usage.Record("often", now)
	}
	usage, _ = usage.Record("once", now)
	for _, c := range []struct {
		id   core.ID
		now  int64
		want uint32
	}{
		{"often", now, 1_000},
		{"once", now, 525},
		{"once", now + day, 275},
		{"never", now, 0},
	} {
		if got := usage.Bonus(c.id, c.now); got != c.want {
			t.Errorf("bonus of %s: %d, want %d", c.id, got, c.want)
		}
	}
	if _, used := usage.Record("once", now-day); used.LastUsed != now {
		t.Errorf("a clock set back made the entry older: %d", used.LastUsed)
	}
}

func TestRankingBreaksTiesByRecencyThenID(t *testing.T) {
	usage := NewUsage(map[core.ID]core.Use{
		"b": {Count: 1, LastUsed: 10},
		"a": {Count: 1, LastUsed: 10},
		"c": {Count: 1, LastUsed: 20},
		"d": {Count: 5, LastUsed: 0},
	})
	if got := usage.Ranked(20); !slices.Equal(got, []core.ID{"d", "c", "a", "b"}) {
		t.Errorf("ranked %q", got)
	}
}

func TestUsageForgetsTheLeastRecentlyUsedPastItsLimit(t *testing.T) {
	entries := map[core.ID]core.Use{}
	for n := range MaxUsage {
		entries[core.ID(fmt.Sprint("app:/", n))] = core.Use{Count: 9, LastUsed: int64(100 + n)}
	}
	// Two equally old entries: the larger ID goes, as the store drops it.
	entries["app:/0"] = core.Use{Count: 9, LastUsed: 50}
	entries["app:/1"] = core.Use{Count: 9, LastUsed: 50}
	usage := NewUsage(entries)
	next, used := usage.Record("app:/new", 5_000)
	if used != (core.Use{Count: 1, LastUsed: 5_000}) {
		t.Errorf("recorded %+v", used)
	}
	ranked := next.Ranked(5_000)
	if len(ranked) != MaxUsage || slices.Contains(ranked, "app:/1") || !slices.Contains(ranked, "app:/0") {
		t.Errorf("%d kept; app:/0 kept %v, app:/1 kept %v", len(ranked), slices.Contains(ranked, "app:/0"), slices.Contains(ranked, "app:/1"))
	}
	if len(usage.Ranked(5_000)) != MaxUsage || usage.Bonus("app:/new", 5_000) != 0 {
		t.Error("Record changed the old value")
	}
	if next.Remove("app:/new").Bonus("app:/new", 5_000) != 0 || next.Bonus("app:/new", 5_000) == 0 {
		t.Error("Remove did not forget, or changed the old value")
	}
}

func TestPinsKeepOrderAndIgnoreDuplicates(t *testing.T) {
	pins := NewPins(nil).Add("a").Add("b").Add("a").Remove("c")
	if !slices.Equal(pins.IDs(), []core.ID{"a", "b"}) {
		t.Fatalf("pins %q", pins.IDs())
	}
	removed := pins.Remove("a")
	if removed.Contains("a") || !pins.Contains("a") || removed.Len() != 1 {
		t.Errorf("Remove: %q, old value %q", removed.IDs(), pins.IDs())
	}
	// Two values made from one must not share what they add.
	base := NewPins(make([]core.ID, 0, 8)).Add("a")
	x, y := base.Add("x"), base.Add("y")
	if !slices.Equal(x.IDs(), []core.ID{"a", "x"}) || !slices.Equal(y.IDs(), []core.ID{"a", "y"}) {
		t.Errorf("shared storage: %q %q", x.IDs(), y.IDs())
	}
}

func TestHiddenAndHotkeysAreValues(t *testing.T) {
	hidden := NewHidden("b")
	more := hidden.Add("a")
	if !slices.Equal(more.IDs(), []core.ID{"a", "b"}) || hidden.Contains("a") || more.Remove("b").Contains("b") || !more.Contains("b") {
		t.Errorf("hidden %q, old value %q", more.IDs(), hidden.IDs())
	}
	hotkeys := NewHotkeys(map[core.ID]string{"system:lock": "Alt+KeyL"})
	changed := hotkeys.Set("app:/a.app", "Alt+KeyA").Remove("system:lock")
	if got := changed.All(); len(got) != 1 || got[0] != (Pair{"app:/a.app", "Alt+KeyA"}) {
		t.Errorf("hotkeys %v", got)
	}
	if shortcut, ok := hotkeys.Get("system:lock"); !ok || shortcut != "Alt+KeyL" || hotkeys.Len() != 1 {
		t.Error("Set or Remove changed the old value")
	}
}
