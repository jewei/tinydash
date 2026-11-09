package search

import (
	"fmt"
	"io/fs"
	"math/rand/v2"
	"slices"
	"testing"
	"testing/fstest"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/apps"
	"github.com/jewei/tinydash/internal/feature/files"
)

var words = []string{
	"alpha", "budget", "camera", "delta", "editor", "finder", "garden", "harbor", "invoice", "journal",
	"kernel", "ledger", "mail", "notes", "orbit", "photo", "query", "report", "studio", "terminal",
	"update", "vector", "weather", "xray", "yield", "zebra", "safari", "slack", "music", "code",
}

// large is the design's worst case: 5,000 apps and 50,000 files, with
// usage, pins, and an alias.
func large() *Snapshot {
	r := rand.New(rand.NewPCG(1, 2))
	name := func(n int) string {
		out := words[r.IntN(len(words))]
		for range n - 1 {
			out += " " + words[r.IntN(len(words))]
		}
		return out
	}
	list := make([]apps.App, apps.MaxApps)
	for i := range list {
		list[i] = apps.App{Name: name(2), Path: fmt.Sprintf("/Applications/%d.app", i)}
	}
	disk := mapDisk{}
	for folder := range 500 {
		for file := range 99 {
			disk[fmt.Sprintf("home/%s %d/%s %d.txt", name(1), folder, name(2), file)] = &fstest.MapFile{}
		}
	}
	s := empty()
	s.Apps = apps.NewIndex(list)
	s.Files = files.Scan(newListedDisk(fstest.MapFS(disk)), files.Options{Roots: []string{"home"}})
	used := map[core.ID]core.Use{}
	for i := range 200 {
		used[core.ID(fmt.Sprintf("app:/Applications/%d.app", i))] = core.Use{Count: uint32(i % 20), LastUsed: s.Now.Unix()}
	}
	s.Usage = NewUsage(used)
	s.Pins = NewPins([]core.ID{"app:/Applications/1.app", "system:lock"})
	s.Aliases = NewAliases(map[core.ID]string{"app:/Applications/2.app": "sa"})
	return s
}

// listedDisk lists each folder of a MapFS once, up front: a MapFS reads
// every file to list one folder, which is too slow for 50,000 files.
type listedDisk map[string][]fs.DirEntry

func newListedDisk(fsys fstest.MapFS) listedDisk {
	d := listedDisk{}
	_ = fs.WalkDir(fsys, ".", func(path string, e fs.DirEntry, err error) error {
		if err == nil && e.IsDir() {
			d[path], err = fs.ReadDir(fsys, path)
		}
		return err
	})
	return d
}

func (d listedDisk) ReadDir(dir string) ([]fs.DirEntry, error) { return d[dir], nil }

func (d listedDisk) RealDir(dir string) (string, bool) {
	_, ok := d[dir]
	return dir, ok
}

// BenchmarkSearch reports the p50 and p95 of one Search per query. The
// design's budget is 8 ms at p95 on the main thread.
func BenchmarkSearch(b *testing.B) {
	s := large()
	if s.Files.Len() != files.Limit {
		b.Fatalf("%d files, want %d", s.Files.Len(), files.Limit)
	}
	for _, c := range []struct {
		query    string
		category core.Category
	}{
		{"", core.CategoryAll},
		{"s", core.CategoryAll},
		{"sa", core.CategoryAll},
		{"report", core.CategoryAll},
		{"rep bud", core.CategoryAll},
		{"zzzz", core.CategoryAll},
		{"12 * 8", core.CategoryAll},
		{"notes", core.CategoryFiles},
		{"code", core.CategoryApps},
	} {
		b.Run(fmt.Sprintf("%s/%q", c.category, c.query), func(b *testing.B) {
			// The package is pure, so the benchmark's own clock times each run.
			var took []time.Duration
			for b.Loop() {
				start := b.Elapsed()
				Search(s, c.query, c.category)
				took = append(took, b.Elapsed()-start)
			}
			slices.Sort(took)
			at := func(p float64) float64 { return float64(took[int(p*float64(len(took)-1))].Microseconds()) / 1000 }
			b.ReportMetric(at(0.50), "p50-ms")
			b.ReportMetric(at(0.95), "p95-ms")
		})
	}
}
