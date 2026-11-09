package search

import (
	"fmt"
	"io/fs"
	"math/rand/v2"
	"slices"
	"strings"
	"testing"
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
	disk := listedDisk{".": {entry{"home", true}}}
	for folder := range 500 {
		dir := fmt.Sprintf("%s %d", name(1), folder)
		disk["home"] = append(disk["home"], entry{dir, true})
		for file := range 99 {
			disk["home/"+dir] = append(disk["home/"+dir], entry{fmt.Sprintf("%s %d.txt", name(2), file), false})
		}
	}
	// A real folder lists its entries by name, as fs.ReadDir does.
	for _, list := range disk {
		slices.SortFunc(list, func(a, b fs.DirEntry) int { return strings.Compare(a.Name(), b.Name()) })
	}
	s := empty()
	s.Apps = apps.NewIndex(list)
	s.Files = files.Scan(disk, files.Options{Roots: []string{"home"}})
	used := map[core.ID]core.Use{}
	for i := range 200 {
		used[core.ID(fmt.Sprintf("app:/Applications/%d.app", i))] = core.Use{Count: uint32(i % 20), LastUsed: s.Now.Unix()}
	}
	s.Usage = NewUsage(used)
	s.Pins = NewPins([]core.ID{"app:/Applications/1.app", "system:lock"})
	s.Aliases = NewAliases(map[core.ID]string{"app:/Applications/2.app": "sa"})
	return s
}

// listedDisk is a disk with each folder listed up front: building 50,000
// files in an fstest.MapFS and listing it takes seconds.
type listedDisk map[string][]fs.DirEntry

// entry is a file or folder of a listedDisk.
type entry struct {
	name string
	dir  bool
}

func (e entry) Name() string { return e.name }
func (e entry) IsDir() bool  { return e.dir }
func (e entry) Type() fs.FileMode {
	if e.dir {
		return fs.ModeDir
	}
	return 0
}
func (e entry) Info() (fs.FileInfo, error) { return nil, fs.ErrInvalid }

func (d listedDisk) ReadDir(dir string) ([]fs.DirEntry, error) { return d[dir], nil }

func (d listedDisk) RealDir(dir string) (string, bool) {
	_, ok := d[dir]
	return dir, ok
}

// searchBudget is the design's limit for one search on the main thread, at
// p95, on the user's machine (an Apple M2 measures about 5 ms at worst).
const searchBudget = 8 * time.Millisecond

// gateP95 is where TestSearchStaysWithinBudget fails: twice the budget,
// because CI runs it on shared machines that are slower than a desk Mac
// and noisy at p95. It still catches a search that becomes about four
// times slower.
const gateP95 = 2 * searchBudget

// budgetQueries are the shapes of search that the budget covers.
var budgetQueries = []struct {
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
}

// timeSearch runs one Search per iteration of b and returns the p50 and p95.
// The package is pure, so the benchmark's own clock times each run.
func timeSearch(b *testing.B, s *Snapshot, query string, category core.Category) (p50, p95 time.Duration) {
	var took []time.Duration
	for b.Loop() {
		start := b.Elapsed()
		Search(s, query, category)
		took = append(took, b.Elapsed()-start)
	}
	slices.Sort(took)
	at := func(p float64) time.Duration { return took[int(p*float64(len(took)-1))] }
	return at(0.50), at(0.95)
}

func largeChecked(tb testing.TB) *Snapshot {
	tb.Helper()
	s := large()
	if s.Files.Len() != files.Limit {
		tb.Fatalf("%d files, want %d", s.Files.Len(), files.Limit)
	}
	return s
}

// BenchmarkSearch reports the p50 and p95 of one Search per query.
func BenchmarkSearch(b *testing.B) {
	s := largeChecked(b)
	for _, c := range budgetQueries {
		b.Run(fmt.Sprintf("%s/%q", c.category, c.query), func(b *testing.B) {
			p50, p95 := timeSearch(b, s, c.query, c.category)
			b.ReportMetric(float64(p50.Microseconds())/1000, "p50-ms")
			b.ReportMetric(float64(p95.Microseconds())/1000, "p95-ms")
		})
	}
}

// TestSearchStaysWithinBudget is the gate on search speed. The race
// detector makes search several times slower, so scripts/verify-go.sh runs
// this test again without it.
func TestSearchStaysWithinBudget(t *testing.T) {
	if raceDetector {
		t.Skip("timing is meaningless under the race detector; verify-go.sh runs this test without it")
	}
	s := largeChecked(t)
	for _, c := range budgetQueries {
		var p95 time.Duration
		testing.Benchmark(func(b *testing.B) { _, p95 = timeSearch(b, s, c.query, c.category) })
		t.Logf("%s %q: p95 %v", c.category, c.query, p95)
		if p95 > gateP95 {
			t.Errorf("%s %q: p95 %v, over the gate of %v (budget %v)", c.category, c.query, p95, gateP95, searchBudget)
		}
	}
}
