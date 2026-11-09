package launcher

import (
	"strings"

	"github.com/egoist/mygo/i18n"
	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// loadsDetails are the kinds whose details the backend loads, as LOADED in
// the Rust app's PreviewPane.tsx.
var loadsDetails = map[core.Kind]bool{
	core.KindApp: true, core.KindClipboard: true, core.KindFile: true, core.KindFolder: true, core.KindSnippet: true,
}

// answers are the kinds whose title is an answer to read and copy, shown
// in the body font rather than the display one.
var answers = map[core.Kind]bool{
	core.KindCalculation: true, core.KindDateTime: true, core.KindPassword: true, core.KindURL: true, core.KindColor: true,
}

// showsDetails reports whether the details pane shows: beside results,
// where the widget pane does not.
func (m *Model) showsDetails() bool { return !m.showsWidgets() && len(m.rows()) > 0 }

// loadDetails asks for the selected result's details when the selection
// changed, or a search ran since the last ask. Only the latest load shows.
func (m *Model) loadDetails() {
	var want core.ID
	if r, ok := m.selectedRow(); ok && m.showsDetails() && loadsDetails[r.Kind] {
		want = r.ID
	}
	if want == m.detailsWanted && !m.detailsStale {
		return
	}
	m.detailsWanted, m.detailsStale = want, false
	m.detailLoads++
	if want == "" {
		return
	}
	load := m.detailLoads
	m.backend.Details(want, func(d Details, ok bool) {
		if load == m.detailLoads {
			m.detailsOf, m.details, m.hasDetails = want, d, ok
		}
	})
}

// detailsPane shows the selected result large: its icon, kind, title, and
// subtitle, what the backend loaded of it, and every action.
func (m *Model) detailsPane(c *ui.Context, p kit.Palette) {
	r, _ := m.selectedRow()
	loaded := m.hasDetails && m.detailsOf == r.ID
	ui.Scroll(c).Grow(9).Padding(20).Gap(16).Background(p.Raised).BorderWidth(0, 0, 0, 1).BorderColor(p.Rule).
		Role(ui.RoleRegion).Label("Details").Children(func() {
		ui.Column(c).Gap(8).AlignItems(ui.Start).Children(func() {
			m.icon(c, p, r, 56)
			ui.Text(c, strings.ToUpper(kinds[r.Kind].label)).FontSize(11).FontWeight(600).TextColor(p.Muted)
			// A copied text's title is its first line, which the text below
			// shows; until the text loads, the title stays hidden too, so
			// it never flashes and goes.
			if r.Kind != core.KindClipboard || loaded && m.details.Kind != engine.DetailsText {
				title := ui.Text(c, r.Title).FontSize(26).TextColor(p.Ink)
				switch {
				case r.Kind == core.KindPassword:
					title.FontSize(20)
				case answers[r.Kind]:
					title.FontWeight(600)
				default:
					title.Font(kit.DisplayFont)
				}
			}
			if r.Subtitle != "" {
				ui.Text(c, r.Subtitle).TextColor(p.Muted)
			}
		})
		if loaded {
			m.loadedDetails(c, p, m.details)
		}
		ui.Box(c).Grow(1)
		ui.Column(c).Gap(2).AlignItems(ui.Stretch).Children(func() {
			for i, ra := range r.Actions {
				b := ui.ButtonBase(c).Padding(6, 8).Radius(6).Justify(ui.Start).Label(ra.Label)
				if b.Hovered() && !m.running {
					b.Background(p.Hover)
				}
				b.Disabled(m.running).Children(func() {
					ui.Row(c).Grow(1).Gap(8).AlignItems(ui.Center).Children(func() {
						ui.Text(c, ra.Label).TextColor(p.Ink).Grow(1)
						if keys := shortcutFor(ra.Action, i); keys != nil {
							keyCaps(c, p, keys...)
						}
					})
				}).OnClick(func() {
					m.runResult(r, i)
					m.focusInput()
				})
			}
		})
	})
}

// detailsImageHeight is the tallest a copied image shows, as in the Rust
// app; a wide one also fits detailsImageWidth.
const detailsImageHeight, detailsImageWidth = 220, 300

func (m *Model) loadedDetails(c *ui.Context, p kit.Palette, d Details) {
	switch d.Kind {
	case engine.DetailsText:
		ui.Scroll(c).MaxHeight(220).Padding(12).Radius(6).Border(1, p.Rule).Background(p.Paper).Children(func() {
			ui.Text(c, d.Text).FontSize(12).TextColor(p.Ink)
		})
	case engine.DetailsImage:
		if d.Image == nil || d.Width == 0 || d.Height == 0 {
			return
		}
		scale := min(1, float32(detailsImageHeight)/float32(d.Height), float32(detailsImageWidth)/float32(d.Width))
		ui.Image(c, d.Image).Size(float32(d.Width)*scale, float32(d.Height)*scale).Radius(6).Border(1, p.Rule)
	case engine.DetailsFiles:
		ui.Column(c).Gap(2).Role(ui.RoleList).Label("Copied files").Children(func() {
			for _, path := range d.Paths {
				ui.Text(c, path).FontSize(12).TextColor(p.Ink).Role(ui.RoleListItem)
			}
		})
	case engine.DetailsFile:
		f := d.File
		ui.Column(c).Gap(4).Children(func() {
			fact(c, p, "Where", f.Path)
			if !f.IsDir {
				fact(c, p, "Size", formatBytes(uint64(max(f.Size, 0))))
			}
			if !f.Modified.IsZero() {
				locale := c.Region().FormatLocale(c.Locale())
				fact(c, p, "Modified", i18n.FormatDate(locale, f.Modified.Local(), i18n.DateOptions{Date: i18n.ShortStyle, Time: i18n.MediumStyle}))
			}
		})
	}
}

func fact(c *ui.Context, p kit.Palette, name, value string) {
	ui.Row(c).Gap(12).Children(func() {
		ui.Text(c, name).FontSize(12).TextColor(p.Muted).Width(64)
		ui.Text(c, value).FontSize(12).TextColor(p.Ink).Grow(1).Shrink(1)
	})
}
