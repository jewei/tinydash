package launcher

import (
	"fmt"
	"math"
	"strconv"
	"time"

	"github.com/egoist/mygo/i18n"
	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// widgetPane shows the widgets that are on, ported from
// src/launcher/WidgetPane.tsx: the clipboard card, Clocks, Weather, Focus
// timer, Disk space, and Notepad. The full JSON view takes the place of the
// pane while it is open.
func (m *Model) widgetPane(c *ui.Context, p kit.Palette) {
	w := m.widgets
	card := m.card()
	if m.showsJSON() {
		m.jsonView(c, p, card)
		return
	}
	// The pane scrolls when its cards do not fit, as the Solid app's did.
	// Its column is at least as tall as the pane was in the last frame, so
	// the note grows into the room that the other cards leave.
	pane := ui.Scroll(c).Grow(9).Background(p.Raised).BorderWidth(0, 0, 0, 1).BorderColor(p.Rule).
		Role(ui.RoleRegion).Label("Widgets")
	room := pane.Bounds().H
	if room != m.paneRoom {
		// A new pane, or a bar above it that came or went, changed its
		// height after the last frame: draw again with the new one.
		c.After(0)
		m.paneRoom = room
	}
	pane.Children(func() {
		ui.Column(c).MinHeight(room).Padding(12).Gap(8).Children(func() { m.widgetCards(c, p, w, card) })
	})
}

// widgetCards are the cards of the widgets that are on, in the Solid
// app's order.
func (m *Model) widgetCards(c *ui.Context, p kit.Palette, w Widgets, card *ClipCard) {
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
	if w.Weather != nil {
		m.weatherCard(c, p, *w.Weather)
	}
	if w.Focus != nil {
		m.focusTimer(c, p, *w.Focus)
	}
	if w.Disk != nil {
		diskSpace(c, p, *w.Disk)
	}
	if w.Note != nil {
		m.notepadCard(c, p, *w.Note)
	}
	// Only clipboard cards are on, and the clipboard has none.
	if card == nil && !w.Clocks && w.Weather == nil && w.Focus == nil && w.Disk == nil && w.Note == nil {
		ui.Text(c, "Copy a color, a Unix time, or JSON to see it here.").Padding(12).FontSize(12).TextColor(p.Muted)
	}
}

// focusTimer shows the phase, a ring of the time left, and the timer's
// controls. Breaks show in calm colors.
func (m *Model) focusTimer(c *ui.Context, p kit.Palette, t FocusTimer) {
	ring, ringInk := p.Accent, p.AccentIn
	if t.Break {
		ring, ringInk = p.Calm, p.CalmInk
	}
	left := t.left(c.Now())
	label, shown := t.Phase, minutesAndSeconds(left)
	switch {
	case t.Done:
		label, shown = "Session done", "Done"
	case t.Paused:
		label += " · Paused"
	}
	progress := float32(1)
	if !t.Done && t.Total > 0 {
		progress = float32(left) / float32(t.Total)
	}
	if !t.EndsAt.IsZero() && left > 0 {
		// A new frame as the second shown turns, not a timer that polls.
		c.After(left - time.Duration(seconds(left)-1)*time.Second)
	}
	card(c, p, "Focus timer", func() {
		ui.Row(c).Gap(6).AlignItems(ui.Center).Children(func() {
			ui.Box(c).Size(6, 6).Radius(999).Background(ring)
			ui.Text(c, label).FontSize(12).FontWeight(600).TextColor(ring).SingleLine().Grow(1).Shrink(1)
			keyCaps(c, p, modKey, "P")
		})
		ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
			ui.Box(c).Size(64, 64).Center().Draw(func(painter *ui.Painter, r ui.Rect) {
				cx, cy, radius := r.X+r.W/2, r.Y+r.H/2, float32(28)
				painter.StrokePath(circle(cx, cy, radius), 5, p.Field)
				if progress > 0 {
					painter.StrokePath(circle(cx, cy, radius).Trim(0, progress), 5, ring)
				}
			}).Children(func() {
				// A session of 100 minutes or more has a third digit, which
				// must fit inside the ring.
				size := float32(16)
				if len(shown) > len("00:00") {
					size = 13
				}
				ui.Text(c, shown).Font(kit.DisplayFont).FontSize(size).TextColor(p.Ink).FontFeatures("tnum").Role(ui.RoleTimer)
			})
			ui.Column(c).Grow(1).Shrink(1).Gap(4).Children(func() {
				for i, a := range t.Actions {
					focusButton(c, p, a.Label, i == 0, ring, ringInk).OnClick(func() { m.controlFocus(a) })
				}
			})
		})
		ui.Text(c, fmt.Sprintf("Session %d of %d", t.Session, t.Sessions)).FontSize(12).TextColor(p.Muted).SingleLine()
	})
}

// focusButton is a control of the focus timer. It keeps the keyboard in
// the search field, so typing goes on there after a click.
func focusButton(c *ui.Context, p kit.Palette, label string, main bool, ring, ringInk ui.Color) ui.Element {
	b := ui.ButtonBase(c).Height(26).Padding(0, 6).Radius(6).Label(label).KeepFocus()
	ink, weight := p.Ink, 400
	switch {
	case main:
		b.Background(ring)
		ink, weight = ringInk, 600
	case b.Hovered():
		b.Border(1, p.Border).Background(p.Hover)
	default:
		b.Border(1, p.Border)
	}
	return b.Children(func() { ui.Text(c, label).FontSize(12).FontWeight(weight).TextColor(ink).SingleLine() })
}

// left is the time left in the phase at now, never more than the phase,
// as focus.Timer.Left has it.
func (t FocusTimer) left(now time.Time) time.Duration {
	if t.EndsAt.IsZero() {
		return t.Remaining
	}
	return min(max(t.EndsAt.Sub(now), 0), t.Total)
}

// seconds are the whole seconds a countdown shows for d: it reads 00:01
// until the last second is over.
func seconds(d time.Duration) int64 { return int64((d + time.Second - 1) / time.Second) }

// minutesAndSeconds writes d as mm:ss, as the Solid app did.
func minutesAndSeconds(d time.Duration) string {
	s := seconds(d)
	return fmt.Sprintf("%02d:%02d", s/60, s%60)
}

// circle is a circle that starts at its top and runs clockwise, so a
// trimmed one fills from twelve o'clock, as the Solid app's ring did.
func circle(cx, cy, r float32) *ui.Path {
	const k = 0.5522847498
	return (&ui.Path{}).MoveTo(cx, cy-r).
		CubeTo(cx+r*k, cy-r, cx+r, cy-r*k, cx+r, cy).
		CubeTo(cx+r, cy+r*k, cx+r*k, cy+r, cx, cy+r).
		CubeTo(cx-r*k, cy+r, cx-r, cy+r*k, cx-r, cy).
		CubeTo(cx-r, cy-r*k, cx-r*k, cy-r, cx, cy-r)
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
