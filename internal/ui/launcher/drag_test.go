package launcher

import (
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// The empty parts of the tab bar and the footer move the panel, which has
// no title bar. A press there starts the window's drag and leaves the
// keyboard in the search field, so typing goes on after the move.
func TestPressingTheEmptyBarsKeepsTheSearchFocused(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tabs, _ := tt.AccessByRole(ui.RoleTabList, "Categories")
	actions, _ := tt.AccessByRole(ui.RoleButton, "Actions")
	for _, at := range []struct {
		name string
		x, y float32
	}{
		{"the tab bar", tabs.Bounds.X + tabs.Bounds.W - 30, tabs.Bounds.Y + tabs.Bounds.H/2},
		{"the footer", actions.Bounds.X - 60, actions.Bounds.Y + actions.Bounds.H/2},
	} {
		tt.Press(at.x, at.y)
		tt.Release(at.x, at.y)
		if !tt.Focused("Search") {
			t.Errorf("a press on %s took the focus from the search field", at.name)
		}
	}
	// Each press told the app that a drag may start, so the blur that a
	// window manager's move causes does not hide the panel.
	if f.drags != 2 {
		t.Errorf("%d drag notices for 2 presses on the bars", f.drags)
	}
}

// Only a press on a bar itself can move the panel. A press on a tab, the
// Actions button, or Clear History is a click: it must not tell the app a
// drag began, or a blur after it waits for a drag that never comes.
func TestPressingABarsControlsStartsNoDrag(t *testing.T) {
	f := newFake(always(clip))
	m, tt := open(t, f)
	m.setCategory(core.CategoryClipboard)
	tt.Frame()
	for _, name := range []string{"Apps", "Actions", "Clear History…"} {
		info, ok := tt.Access(name)
		if !ok {
			t.Fatalf("no %q: %q", name, tt.Texts())
		}
		x, y := info.Bounds.X+info.Bounds.W/2, info.Bounds.Y+info.Bounds.H/2
		tt.Press(x, y)
		tt.Release(x, y)
		if f.drags != 0 {
			t.Fatalf("a press on %s started a drag", name)
		}
		m.closeMenu()
		m.cancelConfirm()
		m.setCategory(core.CategoryClipboard)
		tt.Frame()
	}
}

// A press on a bar that ends without moving the panel is no drag any more,
// so a blur after it hides the panel at once.
func TestReleasingABarPressEndsTheDrag(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tabs, _ := tt.AccessByRole(ui.RoleTabList, "Categories")
	x, y := tabs.Bounds.X+tabs.Bounds.W-30, tabs.Bounds.Y+tabs.Bounds.H/2
	tt.Press(x, y)
	if f.drags != 1 || f.dragEnds != 0 {
		t.Fatalf("drags %d, ends %d after the press", f.drags, f.dragEnds)
	}
	tt.Release(x, y)
	if f.dragEnds != 1 {
		t.Errorf("ends %d after the release", f.dragEnds)
	}
	// A release elsewhere, after no drag, ends nothing.
	click(t, tt, "Apps")
	if f.drags != 1 || f.dragEnds != 1 {
		t.Errorf("drags %d, ends %d after a click on a tab", f.drags, f.dragEnds)
	}
}

// A double press on a bar must not reach mygo's title bar double-click.
func TestADoublePressOnABarIsTakenAndStartsNoDrag(t *testing.T) {
	for _, c := range []struct {
		ev          ui.InputEvent
		drag, taken bool
	}{
		{ui.InputEvent{Kind: ui.InputPointerDown, Button: 0, Clicks: 1}, true, false},
		{ui.InputEvent{Kind: ui.InputPointerDown, Button: 0, Clicks: 2}, false, true},
		{ui.InputEvent{Kind: ui.InputPointerDown, Button: 0, Clicks: 3}, false, true},
		{ui.InputEvent{Kind: ui.InputPointerDown, Button: 1, Clicks: 1}, false, false},
		{ui.InputEvent{Kind: ui.InputPointerUp, Button: 0, Clicks: 2}, false, false},
	} {
		if drag, taken := barPress(c.ev); drag != c.drag || taken != c.taken {
			t.Errorf("%+v: drag %v taken %v, want %v %v", c.ev, drag, taken, c.drag, c.taken)
		}
	}
}
