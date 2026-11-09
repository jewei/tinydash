package emoji

import (
	"slices"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

func first(t *testing.T, x *Index, query string, ctx Context) core.Result {
	t.Helper()
	hits := x.Search(match.New(query), ctx, 5)
	if len(hits) == 0 {
		t.Fatalf("%q finds nothing", query)
	}
	return hits[0].Result
}

func TestFindsByNameAndShortcode(t *testing.T) {
	x := NewIndex()
	rocket := first(t, x, "rocket", Context{})
	if rocket.Icon != core.EmojiIcon("🚀") || rocket.Title != "Rocket" || rocket.Subtitle != ":rocket:" {
		t.Errorf("rocket: %+v", rocket)
	}
	for query, want := range map[string]core.ID{"+1": "emoji:👍", ":rocket:": "emoji:🚀", ":rocket": "emoji:🚀"} {
		if got := first(t, x, query, Context{}).ID; got != want {
			t.Errorf("%q: %s, want %s", query, got, want)
		}
	}
}

func TestLocalizedKeywordsOnlyWhenEnabled(t *testing.T) {
	if hits := NewIndex().Search(match.New("火箭"), Context{}, 5); len(hits) != 0 {
		t.Errorf("火箭 without Chinese: %v", hits)
	}
	x := NewIndex(Chinese, Malay, Spanish)
	for _, query := range []string{"火箭", "roket", "cohete"} {
		if got := first(t, x, query, Context{}).ID; got != "emoji:🚀" {
			t.Errorf("%q: %s", query, got)
		}
	}
}

func TestSkinToneChangesTheCopyButNotTheID(t *testing.T) {
	wave := first(t, NewIndex(), "waving hand", Context{SkinTone: 3})
	if wave.ID != "emoji:👋" || wave.Icon != core.EmojiIcon("👋🏽") {
		t.Errorf("wave: %+v", wave)
	}
	if copied := wave.Actions[0].Action; copied != (core.Copy{Text: "👋🏽"}) {
		t.Errorf("copies %+v", copied)
	}
}

func TestGetFindsAnyFormAndToneOfAnEmoji(t *testing.T) {
	x := NewIndex()
	for glyph, want := range map[string]core.ID{
		"👋🏿":         "emoji:👋",
		"👋":          "emoji:👋",
		"\U0001F43F": "emoji:\U0001F43F\uFE0F", // unqualified chipmunk
	} {
		got, ok := x.Get(glyph, Context{})
		if !ok || got.ID != want {
			t.Errorf("%q: %s %v, want %s", glyph, got.ID, ok, want)
		}
	}
	if _, ok := x.Get("x", Context{}); ok {
		t.Error("x is an emoji")
	}
}

type personal struct {
	boost  map[core.ID]uint32
	pinned map[core.ID]bool
}

func (p personal) Boost(id core.ID) uint32 { return p.boost[id] }
func (p personal) Pinned(id core.ID) bool  { return p.pinned[id] }

func TestBrowseListsMostUsedFirstThenTheCatalog(t *testing.T) {
	ctx := Context{Personal: personal{
		boost:  map[core.ID]uint32{"emoji:🚀": 5, "emoji:🎉": 9, "emoji:👍": 5},
		pinned: map[core.ID]bool{"emoji:🎉": true},
	}}
	results := NewIndex().Browse(ctx, 5)
	var ids []core.ID
	for _, r := range results {
		ids = append(ids, r.ID)
	}
	if want := []core.ID{"emoji:🎉", "emoji:👍", "emoji:🚀", "emoji:😀", "emoji:😃"}; !slices.Equal(ids, want) {
		t.Errorf("%v, want %v", ids, want)
	}
	if party := results[0]; !party.Pinned || party.Actions[1] != core.Do("Unpin", core.Unpin{ID: "emoji:🎉"}) {
		t.Errorf("pinned: %+v", party)
	}
	if thumbs := results[1]; thumbs.Pinned || thumbs.Actions[1] != core.Do("Pin", core.Pin{ID: "emoji:👍"}) {
		t.Errorf("not pinned: %+v", thumbs)
	}
}

func TestUsageRaisesASearchResult(t *testing.T) {
	// 🐈 is "cat" (10,074); 🐱 is "cat face" (9,974).
	ctx := Context{Personal: personal{boost: map[core.ID]uint32{"emoji:🐱": 200}}}
	if got := first(t, NewIndex(), "cat", ctx).ID; got != "emoji:🐱" {
		t.Errorf("cat with usage: %s", got)
	}
}
