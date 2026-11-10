package launcher

import (
	"slices"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// upNext shows the rest of today's events and the reminders due today,
// ported from src/launcher/UpNext.tsx. A click on an item opens Calendar or
// Reminders.
func (m *Model) upNext(c *ui.Context, p kit.Palette, u UpNext) {
	card(c, p, "Up Next", func() {
		ui.Text(c, "Up Next").FontSize(12).FontWeight(600).TextColor(p.Muted)
		if len(u.Items) > 0 {
			ui.Column(c).Gap(0).AlignItems(ui.Stretch).Children(func() {
				for _, it := range u.Items {
					m.upNextItem(c, p, it)
				}
			})
		}
		for _, problem := range u.Problems {
			ui.Text(c, problem).FontSize(12).TextColor(p.Muted)
		}
		if u.Ask != nil {
			ask := *u.Ask
			ui.Row(c).Children(func() {
				ui.Button(c, ask.Label).Disabled(m.running).OnClick(func() {
					m.start(ask, "", "")
					m.focusInput()
				})
			})
		}
		if len(u.Items) == 0 && len(u.Problems) == 0 {
			ui.Text(c, "Nothing else today").FontSize(12).TextColor(p.Muted)
		}
	})
	if wait := m.upNextChanged(c.Now()); wait > 0 {
		c.After(wait)
	}
}

// upNextActions are the actions of Up Next for the actions menu while the
// pane shows, each once: every event opens Calendar.
func (m *Model) upNextActions() []core.ResultAction {
	u := m.widgets.UpNext
	if u == nil || !m.showsWidgets() || m.showsJSON() {
		return nil
	}
	var out []core.ResultAction
	add := func(ra core.ResultAction) {
		if !slices.ContainsFunc(out, func(o core.ResultAction) bool { return o.Label == ra.Label }) {
			out = append(out, ra)
		}
	}
	for _, it := range u.Items {
		add(it.Action)
	}
	if u.Ask != nil {
		add(*u.Ask)
	}
	return out
}

// upNextItem is one row: a time column that keeps the rows lined up, the
// title, and the calendar or list. A reminder gets the open circle of a
// task.
func (m *Model) upNextItem(c *ui.Context, p kit.Palette, it UpNextItem) {
	b := ui.ButtonBase(c).Padding(3, 4).Radius(6).Justify(ui.Start).
		Label(it.Time + " " + it.Title + " " + it.Source).Description(it.Action.Label)
	if b.Hovered() && !m.running {
		b.Background(p.Hover)
	}
	b.Disabled(m.running).Children(func() {
		ui.Row(c).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
			ui.Text(c, it.Time).Width(76).FontSize(12).TextColor(p.Muted).FontFeatures("tnum").SingleLine()
			ui.Row(c).Grow(1).Shrink(1).Gap(6).AlignItems(ui.Center).Children(func() {
				if it.Kind == core.UpNextReminder {
					ui.Box(c).Width(8).Height(8).Radius(999).Border(1.5, p.Muted)
				}
				ui.Text(c, it.Title).TextColor(p.Ink).Grow(1).Shrink(1).SingleLine()
			})
			ui.Text(c, it.Source).MaxWidth(120).FontSize(12).TextColor(p.Muted).SingleLine()
		})
	}).OnClick(func() {
		m.start(it.Action, it.Title, "")
		m.focusInput()
	})
}
