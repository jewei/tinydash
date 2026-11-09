package launcher

import (
	"fmt"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// center is the middle of the element with role and name.
func center(t *testing.T, tt *ui.Tester, role ui.Role, name string) (float32, float32) {
	t.Helper()
	info, ok := tt.AccessByRole(role, name)
	if !ok {
		t.Fatalf("no %v %q; texts %q", role, name, tt.Texts())
	}
	return info.Bounds.X + info.Bounds.W/2, info.Bounds.Y + info.Bounds.H/2
}

func TestMovingThePointerSelectsTheRowUnderIt(t *testing.T) {
	f := newFake(always(safari, app("Notes")))
	_, tt := open(t, f)
	x, y := center(t, tt, ui.RoleOption, "Notes")
	tt.Move(x, y)
	if got := selectedRow(tt, "Safari", "Notes"); got != "Notes" {
		t.Fatalf("selected %q after the pointer moved onto Notes", got)
	}
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{app("Notes").Actions[0].Action, app("Notes").ID})
}

// A still pointer over a row must not take the selection back from the
// keyboard, as when the list scrolls or new results come under it.
func TestAStillPointerNeverSelects(t *testing.T) {
	var rows []core.Result
	for i := range 30 {
		rows = append(rows, app(fmt.Sprintf("App %02d", i)))
	}
	f := newFake(always(rows...))
	m, tt := open(t, f)
	x, y := center(t, tt, ui.RoleOption, "App 02")
	tt.Move(x, y)
	if m.selected != 2 {
		t.Fatalf("selected %d after the pointer moved onto row 2", m.selected)
	}
	tt.Key(0, ui.KeyUp)
	tt.Scroll(x, y, 0, 52*5)
	tt.Frame()
	if m.selected != 1 {
		t.Errorf("selected %d after scrolling under a still pointer, want 1", m.selected)
	}
	tt.Type("app")
	if m.selected != 0 {
		t.Errorf("selected %d after new results came under a still pointer, want 0", m.selected)
	}
}

func TestMovingThePointerOverAMenuItemMakesItActive(t *testing.T) {
	f := newFake(always(safari))
	m, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	x, y := center(t, tt, ui.RoleOption, "Show in Finder")
	tt.Move(x, y)
	if m.menu == nil || m.menu.active != 1 {
		t.Fatalf("menu %+v after the pointer moved onto its second item", m.menu)
	}
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{safari.Actions[1].Action, ""})
}
