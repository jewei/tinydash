package clipboard

import (
	"slices"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/match"
)

func textEntry(id int64, text string) Entry {
	c := Text(text)
	return Entry{ID: id, Kind: c.Kind, Title: c.Title(), Text: text}
}

type pins map[core.ID]bool

func (pins) Boost(core.ID) uint32     { return 0 }
func (p pins) Pinned(id core.ID) bool { return p[id] }

func TestSearchNeedsEveryWordAndPrefersNewerEntries(t *testing.T) {
	h := NewHistory([]Entry{textEntry(2, "Meeting notes for Monday"), textEntry(1, "monday standup notes")})
	hits := h.Search(match.New("notes MONDAY"), Context{}, 10)
	var ids []core.ID
	for _, hit := range hits {
		ids = append(ids, hit.Result.ID)
	}
	if !slices.Equal(ids, []core.ID{"clip:2", "clip:1"}) || hits[0].Score <= hits[1].Score {
		t.Fatalf("ids %q, scores %d %d", ids, hits[0].Score, hits[1].Score)
	}
	actions := hits[0].Result.Actions
	if len(actions) != 3 || actions[0].Action != (core.CopyClip{ID: 2}) || actions[1].Action != (core.Pin{ID: "clip:2"}) || actions[2].Action != (core.DeleteClip{ID: 2}) {
		t.Errorf("actions %+v", actions)
	}
	if hits := h.Search(match.New("friday"), Context{}, 10); len(hits) != 0 {
		t.Errorf("%d hits for friday", len(hits))
	}
	if hits := h.Search(match.New("notes"), Context{}, 1); len(hits) != 1 || hits[0].Score != 1_000 {
		t.Errorf("limit 1: %+v", hits)
	}
}

func TestSearchMatchesTheTitleAndEitherUnicodeForm(t *testing.T) {
	files := Files([]string{"/a/Café.txt"})
	h := NewHistory([]Entry{{ID: 1, Kind: KindFiles, Title: files.Title(), Text: "/a/Café.txt"}})
	if hits := h.Search(match.New("café"), Context{}, 10); len(hits) != 1 {
		t.Errorf("%d hits", len(hits))
	}
}

func TestCopiedFilesAlsoCopyAsPlainText(t *testing.T) {
	c := Files([]string{"/a/one.txt"})
	h := NewHistory([]Entry{{ID: 3, Kind: c.Kind, Title: c.Title(), Text: "/a/one.txt"}})
	r, ok := h.Get(3, Context{Personal: pins{"clip:3": true}})
	if !ok {
		t.Fatal("not found")
	}
	var labels []string
	for _, a := range r.Actions {
		labels = append(labels, a.Label)
	}
	if want := []string{"Copy", "Unpin", "Copy as Plain Text", "Delete"}; !slices.Equal(labels, want) {
		t.Errorf("labels %q", labels)
	}
	if r.Actions[2].Action != (core.CopyClipText{ID: 3}) || !r.Pinned || r.Icon.Symbol != core.SymbolFiles {
		t.Errorf("%+v", r)
	}
	if _, ok := h.Get(4, Context{}); ok {
		t.Error("found a missing entry")
	}
}

func TestSubtitlesSayTheKindAndAge(t *testing.T) {
	h := NewHistory([]Entry{
		{ID: 2, Kind: KindImage, Title: "Image 1 × 1", CopiedAt: 1_000},
		{ID: 1, Kind: KindText, Title: "hi", Text: "hi", CopiedAt: 1_000 - 7_200},
	})
	var subtitles []string
	for _, r := range h.Browse(Context{Now: 1_000}) {
		subtitles = append(subtitles, r.Subtitle)
	}
	if want := []string{"Image · Just now", "Text · 2 hours ago"}; !slices.Equal(subtitles, want) {
		t.Errorf("subtitles %q", subtitles)
	}
}

func TestDescribesAge(t *testing.T) {
	for seconds, want := range map[int64]string{
		-5: "Just now", 5: "Just now", 60: "1 minute ago", 7_200: "2 hours ago", 3 * 86_400: "3 days ago",
	} {
		if got := ago(seconds); got != want {
			t.Errorf("%d: %q, want %q", seconds, got, want)
		}
	}
}
