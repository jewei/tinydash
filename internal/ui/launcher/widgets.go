package launcher

import (
	"math"
	"strconv"
	"time"

	"github.com/egoist/mygo/i18n"
	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// widgetPane shows the widgets that are on, ported from
// src/launcher/WidgetPane.tsx: the clipboard card, Clocks, Up Next, Disk
// space, and Notepad. The full JSON view takes the place of the pane while
// it is open.
func (m *Model) widgetPane(c *ui.Context, p kit.Palette) {
	w := m.widgets
	card := m.card()
	if m.showsJSON() {
		m.jsonView(c, p, card)
		return
	}
	ui.Column(c).Grow(9).Padding(12).Gap(8).Background(p.Raised).BorderWidth(0, 0, 0, 1).BorderColor(p.Rule).
		Role(ui.RoleRegion).Label("Widgets").Children(func() {
		if card != nil {
			m.clipCard(c, p, card)
		}
		if w.Clocks {
			// c.Now is the frame's time, which a Tester moves; the local
			// zone is read each frame, so the clock follows a change of it.
			now := c.Now().Local()
			clocks(c, p, now, w.Cities, c.Region().FormatLocale(c.Locale()))
			// A new frame as the minute turns, not a timer that polls.
			c.After(now.Truncate(time.Minute).Add(time.Minute).Sub(now))
		}
		if w.UpNext != nil {
			m.upNext(c, p, *w.UpNext)
		}
		if w.Disk != nil {
			diskSpace(c, p, *w.Disk)
		}
		if w.Note != nil {
			m.notepadCard(c, p, *w.Note)
		}
		// Only clipboard cards are on, and the clipboard has none.
		if card == nil && !w.Clocks && w.UpNext == nil && w.Disk == nil && w.Note == nil {
			ui.Text(c, "Copy a color, a Unix time, or JSON to see it here.").Padding(12).FontSize(12).TextColor(p.Muted)
		}
	})
}

// card is one widget, named for assistive technology.
func card(c *ui.Context, p kit.Palette, name string, children func()) ui.Element {
	return ui.Column(c).Padding(10, 12).Gap(6).Radius(10).Border(1, p.Rule).Background(p.Paper).
		Role(ui.RoleGroup).Label(name).Children(children)
}

// clocks shows local time large on its own, or beside up to three cities.
// A city's time is UTC shifted by its offset, so a fixed offset such as
// UTC+8 works too. Dates follow locale, the user's regional formats, as
// the Rust app's Intl did; times are 24-hour everywhere, as there.
func clocks(c *ui.Context, p kit.Palette, now time.Time, cities []City, locale string) {
	date := func(skeleton string) string {
		return i18n.FormatDate(locale, now, i18n.DateOptions{Skeleton: skeleton})
	}
	card(c, p, "Clocks", func() {
		if len(cities) == 0 {
			ui.Row(c).AlignItems(ui.End).Justify(ui.SpaceBetween).Gap(12).Children(func() {
				ui.Text(c, now.Format("15:04")).Font(kit.DisplayFont).FontSize(46).TextColor(p.Ink).FontFeatures("tnum")
				ui.Column(c).AlignItems(ui.End).Children(func() {
					ui.Text(c, date("EEEE")).FontWeight(600).TextColor(p.Ink)
					ui.Text(c, date("yMMMMd")).FontSize(12).TextColor(p.Muted)
				})
			})
			return
		}
		ui.Row(c).Gap(12).AlignItems(ui.Center).Children(func() {
			ui.Column(c).Gap(4).Padding(0, 12, 0, 0).BorderWidth(0, 1, 0, 0).BorderColor(p.Rule).Children(func() {
				ui.Text(c, now.Format("15:04")).Font(kit.DisplayFont).FontSize(30).TextColor(p.Ink).FontFeatures("tnum")
				ui.Text(c, date("MMMEd")).FontSize(12).TextColor(p.Muted)
			})
			ui.Row(c).Grow(1).Gap(8).Children(func() {
				for _, city := range cities {
					there := now.UTC().Add(time.Duration(city.OffsetSeconds) * time.Second)
					ui.Column(c).Grow(1).Children(func() {
						ui.Text(c, city.Name).FontSize(12).TextColor(p.Muted).SingleLine()
						ui.Text(c, there.Format("15:04")).FontWeight(600).TextColor(p.Ink).FontFeatures("tnum")
						ui.Text(c, city.Difference).FontSize(12).TextColor(p.Muted)
					})
				}
			})
		})
	})
}

// diskSpace shows the free space of the home folder's disk and how full
// it is, in red below a tenth free.
func diskSpace(c *ui.Context, p kit.Palette, d Disk) {
	card(c, p, "Disk space", func() {
		if d.Err != nil {
			ui.Text(c, "Could not read the disk. "+d.Err.Error()).FontSize(12).TextColor(p.Muted)
			return
		}
		low := d.low()
		ui.Row(c).Gap(6).AlignItems(ui.Center).Children(func() {
			ui.Icon(c, glyphs["disk"]).FontSize(14).TextColor(p.Muted)
			ui.Text(c, d.Name).FontWeight(600).TextColor(p.Ink).SingleLine()
			if low {
				ui.Row(c).Gap(4).AlignItems(ui.Center).Children(func() {
					ui.Icon(c, glyphs["warning"]).FontSize(12).TextColor(p.Danger)
					ui.Text(c, "Low space").FontSize(12).FontWeight(600).TextColor(p.Danger)
				})
			}
			ui.Box(c).Grow(1)
			ui.Text(c, formatBytes(d.FreeBytes)).FontSize(12).FontWeight(600).TextColor(p.Ink)
			ui.Text(c, "free of "+formatBytes(d.TotalBytes)).FontSize(12).TextColor(p.Muted)
		})
		used := d.usedPercent()
		fill := p.Accent
		if low {
			fill = p.Danger
		}
		ui.Box(c).Height(6).Radius(999).Background(p.Field).Clip().
			Role(ui.RoleMeter).Label("Disk used").Range(0, 100, float64(used)).Children(func() {
			ui.Box(c).Height(6).WidthPercent(float32(used)).Radius(999).Background(fill)
		})
	})
}

// low reports whether less than a tenth of the disk is free.
func (d Disk) low() bool { return d.FreeBytes*10 < d.TotalBytes }

// usedPercent is how full the disk is, from 0 to 100.
func (d Disk) usedPercent() int {
	if d.TotalBytes == 0 {
		return 0
	}
	return int(math.Round((1 - float64(d.FreeBytes)/float64(d.TotalBytes)) * 100))
}

// formatBytes writes a size in decimal units, as Finder does: one decimal
// below 10, judged after rounding, so 9,960 bytes reads 10 KB.
func formatBytes(bytes uint64) string {
	if bytes < 1000 {
		if bytes == 1 {
			return "1 byte"
		}
		return strconv.FormatUint(bytes, 10) + " bytes"
	}
	units := []string{"KB", "MB", "GB", "TB"}
	value := float64(bytes) / 1000
	unit := 0
	// Compare the rounded value, so 999,999 bytes reads 1.0 MB, not 1000 KB.
	for shown(value) >= 1000 && unit < len(units)-1 {
		value /= 1000
		unit++
	}
	decimals := 0
	if shown(value) < 10 {
		decimals = 1
	}
	return strconv.FormatFloat(shown(value), 'f', decimals, 64) + " " + units[unit]
}

// shown is value as formatBytes writes it: to tenths below 10, else whole.
func shown(value float64) float64 {
	if tenths := math.Round(value*10) / 10; tenths < 10 {
		return tenths
	}
	return math.Round(value)
}
