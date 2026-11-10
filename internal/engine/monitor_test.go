package engine

import (
	"testing"

	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/settings"
)

// fakeClipboard is the OS clipboard, with or without a change counter.
type fakeClipboard struct {
	text    string
	changes uint64
	counted bool
	onRead  func() // runs between the epoch read and the content read
	fail    int    // reads that fail before the next one works
}

func (f *fakeClipboard) copy(text string)      { f.text, f.changes = text, f.changes+1 }
func (f *fakeClipboard) count() (uint64, bool) { return f.changes, f.counted }
func (f *fakeClipboard) read() (clipboard.Content, clipboard.Read) {
	if f.onRead != nil {
		f.onRead()
	}
	if f.fail > 0 {
		f.fail--
		return clipboard.Content{}, clipboard.ReadFailed
	}
	if f.text == "" {
		return clipboard.Content{}, clipboard.ReadNothing
	}
	return clipboard.Text(f.text), clipboard.ReadContent
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
	setHistory(t, e, false)
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
	setHistory(t, e, true)
	cb.copy("kept")
	tick(t, m, 3)
	if reads != 1 || clips(t, e) != 1 {
		t.Fatalf("after turning history on: %d reads, %d clips", reads, clips(t, e))
	}
}

// setHistory turns clipboard history on or off as the Settings window does.
func setHistory(t *testing.T, e *Engine, on bool) {
	t.Helper()
	_, _, file := LoadSettings(t.TempDir())
	if err := <-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = on }); err != nil {
		t.Fatal(err)
	}
}

// What the user copies while history is off is never saved, also where the
// OS has no change counter (Linux) and the monitor compares content.
func TestCopiesMadeWhileHistoryIsOffAreNeverSaved(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		setHistory(t, e, false)
		tick(t, m, 1)
		cb.copy("secret")
		tick(t, m, 2)
		setHistory(t, e, true)
		tick(t, m, 3)
		tick(t, m, 4)
		if n := clips(t, e); n != 0 {
			t.Fatalf("counted %v: %d clips from a copy made while history was off", counted, n)
		}
		cb.copy("new")
		tick(t, m, 5)
		if n := clips(t, e); n != 1 {
			t.Fatalf("counted %v: %d clips after a copy made with history on", counted, n)
		}
	}
}

// A copy the user cleared does not come back when history goes off and on.
func TestAClearedCopyDoesNotComeBackAfterHistoryGoesOffAndOn(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		cb.copy("password")
		tick(t, m, 2)
		if n := clips(t, e); n != 1 {
			t.Fatalf("counted %v: %d clips after the copy", counted, n)
		}
		<-e.ClearClipboard()
		setHistory(t, e, false)
		tick(t, m, 3)
		setHistory(t, e, true)
		tick(t, m, 4)
		tick(t, m, 5)
		if n := clips(t, e); n != 0 {
			t.Fatalf("counted %v: %d clips: the cleared copy came back", counted, n)
		}
	}
}

// History goes off and on again, and the user copies in between, all before
// the next tick: the monitor never saw the off period, but the copy is
// still not saved. A copy made later is.
func TestACopyMadeInAShortOffPeriodIsNeverSaved(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		setHistory(t, e, false)
		cb.copy("secret")
		setHistory(t, e, true)
		tick(t, m, 2)
		tick(t, m, 3)
		if n := clips(t, e); n != 0 {
			t.Fatalf("counted %v: %d clips from a copy made while history was off", counted, n)
		}
		cb.copy("later")
		tick(t, m, 4)
		if n := clips(t, e); n != 1 {
			t.Fatalf("counted %v: %d clips after a later copy", counted, n)
		}
	}
}

// A copy made right after a Clear is kept: only a switch-off makes the next
// read a baseline.
func TestACopyRightAfterAClearIsKept(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		<-e.ClearClipboard()
		cb.copy("fresh")
		tick(t, m, 2)
		if n := clips(t, e); n != 1 {
			t.Fatalf("counted %v: %d clips after a copy following a clear", counted, n)
		}
	}
}

// The model's trace (formal/ClipboardHistory.tla, counted config): a copy
// lands between the tick's counter read and its content read. That read is
// dropped, so the copy is saved once, by the next tick; saved during the
// read, the next tick would save it again, also after a Clear, and a
// cleared copy would come back.
func TestACopyDuringATickIsSavedOnceByTheNextTick(t *testing.T) {
	e, _ := start(t)
	cb := &fakeClipboard{counted: true}
	m := NewMonitor(e, cb.count, cb.read)
	tick(t, m, 1)
	cb.copy("one")
	once := true
	cb.onRead = func() {
		if once {
			once = false
			cb.copy("two")
		}
	}
	tick(t, m, 2)
	if n := clips(t, e); n != 0 {
		t.Fatalf("%d clips from a read during which the clipboard changed", n)
	}
	tick(t, m, 3)
	tick(t, m, 4)
	if n := clips(t, e); n != 1 {
		t.Fatalf("%d clips after the next ticks, want the copy once", n)
	}
}

// A read that fails once, as when the clipboard's owner does not answer in
// time, with no new copy: the unchanged text was seen already and is not
// saved again, so a cleared copy does not come back.
func TestAFailedReadDoesNotBringAClearedCopyBack(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		cb.copy("password")
		tick(t, m, 2)
		<-e.ClearClipboard()
		cb.fail = 1
		tick(t, m, 3)
		tick(t, m, 4)
		if n := clips(t, e); n != 0 {
			t.Fatalf("counted %v: %d clips: a failed read brought the cleared copy back", counted, n)
		}
	}
}

// A first look whose read fails takes nothing as seen: the next read that
// works is the baseline. Else on Linux the content at startup, or a copy
// made while history was off, would be saved by the next tick.
func TestAFailedFirstLookKeepsTheBaselinePending(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{text: "there at startup", counted: counted, fail: 1}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		tick(t, m, 2)
		if n := clips(t, e); n != 0 {
			t.Errorf("counted %v: %d clips: the content at startup was saved", counted, n)
		}

		e, _ = start(t)
		cb = &fakeClipboard{text: "before", counted: counted}
		m = NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		setHistory(t, e, false)
		tick(t, m, 2)
		cb.copy("copied while off")
		tick(t, m, 3)
		setHistory(t, e, true)
		cb.fail = 1
		tick(t, m, 4)
		tick(t, m, 5)
		if n := clips(t, e); n != 0 {
			t.Errorf("counted %v: %d clips: a copy made while history was off was saved", counted, n)
		}
	}
}

// An empty clipboard at startup is a baseline like any other: the first
// copy after it is saved.
func TestTheFirstCopyAfterAnEmptyStartIsSaved(t *testing.T) {
	for _, counted := range []bool{true, false} {
		e, _ := start(t)
		cb := &fakeClipboard{counted: counted}
		m := NewMonitor(e, cb.count, cb.read)
		tick(t, m, 1)
		cb.copy("first copy")
		tick(t, m, 2)
		if n := clips(t, e); n != 1 {
			t.Errorf("counted %v: %d clips, want the first copy", counted, n)
		}
	}
}
