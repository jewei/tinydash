package app

import (
	"slices"
	"testing"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

func TestTheMessageSitsCenteredFourFifthsDownTheWorkArea(t *testing.T) {
	for _, c := range []struct {
		area mygo.Rectangle
		x, y int
	}{
		{mygo.Rectangle{X: 0, Y: 25, Width: 1440, Height: 875}, 640, 25 + 700 - 22},
		// A display left of the primary one, as in the Rust tests.
		{mygo.Rectangle{X: -1920, Y: 0, Width: 1920, Height: 1080}, -1920 + 880, 864 - 22},
	} {
		if x, y := hudSpot(c.area, hudWidth, hudHeight); x != c.x || y != c.y {
			t.Errorf("%+v: at %d,%d, want %d,%d", c.area, x, y, c.x, c.y)
		}
	}
}

// fakeHUDWindow records what the hud does to its panel. Update runs fn at
// once, as the main thread would later.
type fakeHUDWindow struct{ calls []string }

func (w *fakeHUDWindow) Update(fn func())     { fn() }
func (w *fakeHUDWindow) SetPosition(x, y int) { w.calls = append(w.calls, "position") }
func (w *fakeHUDWindow) ShowInactive()        { w.calls = append(w.calls, "show inactive") }
func (w *fakeHUDWindow) SetIgnoreMouseEvents(ignore bool) {
	w.calls = append(w.calls, "pass the pointer")
}
func (w *fakeHUDWindow) Hide() { w.calls = append(w.calls, "hide") }

// fakeTimers holds what hud.after was asked to run.
type fakeTimers struct{ due []func() }

func (f *fakeTimers) after(d time.Duration, fn func()) { f.due = append(f.due, fn) }

func (f *fakeTimers) run() {
	due := f.due
	f.due = nil
	for _, fn := range due {
		fn()
	}
}

func TestTheMessageShowsWithoutFocusFadesAndHides(t *testing.T) {
	w, timers := &fakeHUDWindow{}, &fakeTimers{}
	h := &hud{window: w, after: timers.after, area: func() mygo.Rectangle { return mygo.Rectangle{Width: 800, Height: 600} }}
	tt := ui.NewTester(h.view, hudWidth, hudHeight)
	h.show("Copied")
	tt.Frame() // the window's Update draws again after fn
	if !tt.HasText("Copied") {
		t.Errorf("texts %q", tt.Texts())
	}
	if want := []string{"position", "show inactive", "pass the pointer"}; !slices.Equal(w.calls, want) {
		t.Errorf("calls %q, want %q", w.calls, want)
	}
	if h.fading {
		t.Error("the message fades at once")
	}
	w.calls = nil
	timers.run()
	if !h.fading || !slices.Equal(w.calls, []string{"hide"}) {
		t.Errorf("after its time: fading %v, calls %q", h.fading, w.calls)
	}
}

func TestOnlyTheNewestMessageHidesThePanel(t *testing.T) {
	w, timers := &fakeHUDWindow{}, &fakeTimers{}
	h := &hud{window: w, after: timers.after, area: func() mygo.Rectangle { return mygo.Rectangle{} }}
	h.show("Copied")
	first := timers.due
	timers.due = nil
	h.show("Copied Path")
	w.calls = nil
	for _, fn := range first {
		fn()
	}
	if h.fading || len(w.calls) != 0 {
		t.Errorf("the first message's timers acted on the second: fading %v, calls %q", h.fading, w.calls)
	}
	if h.message != "Copied Path" {
		t.Errorf("message %q", h.message)
	}
}
