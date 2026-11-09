package launcher

import (
	"testing"

	"github.com/egoist/mygo/ui"
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
}
