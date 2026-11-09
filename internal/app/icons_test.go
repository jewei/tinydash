package app

import (
	"strconv"
	"sync"
	"testing"

	"github.com/egoist/mygo/ui"
)

// The icon cache keeps at most maxFileIcons icons, so a long session of
// file searches does not grow it without end (AGENTS.md rule 8). It keeps
// the newest.
func TestTheFileIconCacheIsBounded(t *testing.T) {
	var wg sync.WaitGroup
	c := iconCache{
		load:   func(string) *ui.Bitmap { return &ui.Bitmap{} },
		loaded: wg.Done,
	}
	paths := maxFileIcons + 10
	for i := range paths {
		wg.Add(1)
		c.get(strconv.Itoa(i))
		wg.Wait() // one at a time, so the newest is known
	}
	if len(c.icons) > maxFileIcons {
		t.Errorf("%d icons kept, the limit is %d", len(c.icons), maxFileIcons)
	}
	if c.get(strconv.Itoa(paths-1)) == nil {
		t.Error("the newest icon is gone")
	}
}
