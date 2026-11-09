package engine

import (
	"testing"

	"github.com/jewei/tinydash/internal/feature/clipboard"
)

// fakeClipboard is the OS clipboard, with or without a change counter.
type fakeClipboard struct {
	text    string
	changes uint64
	counted bool
	onRead  func() // runs between the epoch read and the content read
}

func (f *fakeClipboard) copy(text string)      { f.text, f.changes = text, f.changes+1 }
func (f *fakeClipboard) count() (uint64, bool) { return f.changes, f.counted }
func (f *fakeClipboard) read() (clipboard.Content, bool) {
	if f.onRead != nil {
		f.onRead()
	}
	return clipboard.Text(f.text), f.text != ""
}

func tick(t *testing.T, m *Monitor, now int64) {
	t.Helper()
	if done := m.Tick(now); done != nil {
		if err := <-done; err != nil {
			t.Fatal(err)
		}
	}
}

func TestTheMonitorSavesEachCopyOnceAndNotWhatWasThereAtStart(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{text: "there before", counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		tick(t, m, 2)
		if n := clips(t, e); n != 0 {
			t.Fatalf("counted %v: saved what was there at start (%d clips)", counted, n)
		}
		cb.copy("one")
		tick(t, m, 3)
		tick(t, m, 4)
		cb.copy("two")
		tick(t, m, 5)
		if n := clips(t, e); n != 2 {
			t.Fatalf("counted %v: %d clips after two copies", counted, n)
		}
	}
}

// The user clears the history while the monitor is between its epoch read
// and its clipboard read: the content must not come back.
func TestAClearDuringATickWins(t *testing.T) {
	e, _ := start(t)
	cb := &fakeClipboard{counted: true}
	m := NewMonitor(e, cb.count, cb.read)
	tick(t, m, 1)
	cb.copy("secret")
	cb.onRead = func() { <-e.ClearClipboard() }
	tick(t, m, 2)
	if n := clips(t, e); n != 0 {
		t.Fatalf("%d clips after a clear during the tick", n)
	}
}

// While history is off, the monitor reads no content at all.
func TestTheMonitorReadsNothingWhileHistoryIsOff(t *testing.T) {
	e, _ := start(t)
	<-e.SetClipboardHistory(false)
	cb := &fakeClipboard{counted: true}
	reads := 0
	cb.onRead = func() { reads++ }
	m := NewMonitor(e, cb.count, cb.read)
	tick(t, m, 1)
	cb.copy("private")
	tick(t, m, 2)
	if reads != 0 {
		t.Fatalf("%d reads while history is off", reads)
	}
	<-e.SetClipboardHistory(true)
	cb.copy("kept")
	tick(t, m, 3)
	if reads != 1 || clips(t, e) != 1 {
		t.Fatalf("after turning history on: %d reads, %d clips", reads, clips(t, e))
	}
}
