package launcher

import (
	"strconv"
	"strings"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// The clipboard card and the full JSON view, ported from
// src/launcher/ClipCard.tsx.

// cardKeys run the card's main action.
var cardKeys = []string{modKey, "⇧", "↩"}

var cardGlyphs = map[string]*ui.SVG{
	"clipboard": glyph(`<rect x="7" y="3" width="10" height="4" rx="1"/><path d="M8 5H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2"/>`),
	"copy":      glyph(`<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>`),
	"close":     glyph(`<path d="M6 6l12 12M18 6L6 18"/>`),
	"back":      glyph(`<path d="M15 6l-6 6 6 6"/>`),
}

// monoFont is the system's monospace family, for values and JSON.
const monoFont = "monospace"

// card is the clipboard card that shows, or nil: none while the pane is
// hidden, the clipboard holds none, or the user dismissed it.
func (m *Model) card() *ClipCard {
	c := m.widgets.Card
	if c == nil || c.ID == m.dismissedCard || !m.showsWidgets() {
		return nil
	}
	return c
}

// showsJSON reports whether the full JSON view covers the pane.
func (m *Model) showsJSON() bool {
	c := m.card()
	return c != nil && c.JSON != nil && m.jsonFor == c.ID
}

// runCard runs the card's main action: it opens the full JSON view, or
// copies a color or a time in its first form.
func (m *Model) runCard() {
	c := m.card()
	switch {
	case c == nil:
	case c.JSON != nil:
		m.openJSON(c)
	case len(c.Actions) > 0:
		m.start(c.Actions[0], "", "")
	}
}

func (m *Model) dismissCard() {
	if c := m.card(); c != nil {
		m.dismissedCard = c.ID
	}
	m.jsonFor = ""
}

// openJSON opens the full view of c at its first line.
func (m *Model) openJSON(c *ClipCard) {
	m.jsonFor = c.ID
	m.jsonList.ScrollTo(0, ui.Start)
}

// mainLabel names what the card's keys do.
func mainLabel(c *ClipCard) string {
	if c.JSON != nil {
		return "View Full JSON"
	}
	if len(c.Actions) > 0 {
		return c.Actions[0].Label
	}
	return ""
}

// cardMenuItems are the card's actions in the actions menu, the main one
// with its keys.
func (m *Model) cardMenuItems() []menuItem {
	c := m.card()
	if c == nil {
		return nil
	}
	var items []menuItem
	if c.JSON != nil {
		// The card the menu showed: a newer one has items of its own.
		items = append(items, menuItem{label: mainLabel(c), keys: cardKeys, run: func() { m.openJSON(c) }})
	}
	for i, ra := range c.Actions {
		var keys []string
		if i == 0 && c.JSON == nil {
			keys = cardKeys
		}
		items = append(items, menuItem{label: ra.Label, keys: keys, run: func() { m.start(ra, "", "") }})
	}
	return items
}

// cardKind names what the clipboard holds.
func cardKind(c *ClipCard) string {
	switch {
	case c.Color != nil:
		return "Color"
	case c.Time != nil:
		return "Unix time"
	}
	return "JSON"
}

// jsonSummary is "Object · 8 keys · depth 3 · 312 bytes".
func jsonSummary(j *ClipJSON) string {
	kind, noun := "Object", "key"
	if j.Array {
		kind, noun = "Array", "item"
	}
	if j.Count != 1 {
		noun += "s"
	}
	return kind + " · " + strconv.Itoa(j.Count) + " " + noun + " · depth " + strconv.Itoa(j.Depth) + " · " + formatBytes(uint64(j.Bytes))
}

// clipCard shows the card, with its main keys and a Dismiss button.
func (m *Model) clipCard(c *ui.Context, p kit.Palette, cc *ClipCard) {
	kind := cardKind(cc)
	ui.Column(c).Padding(8, 12, 12).Gap(6).Radius(10).Border(1, p.Edge).Background(p.Notice).Clip().
		Role(ui.RoleGroup).Label(kind + " on the clipboard").Children(func() {
		ui.Row(c).Gap(6).AlignItems(ui.Center).Children(func() {
			ui.Icon(c, cardGlyphs["clipboard"]).FontSize(13).TextColor(p.Accent)
			ui.Text(c, strings.ToUpper(kind)).FontSize(12).FontWeight(700).TextColor(p.Accent).SingleLine()
			if cc.Color != nil && cc.Color.CopiedAs != "" {
				badge(c, p, "from "+cc.Color.CopiedAs)
			}
			ui.Box(c).Grow(1)
			keyCaps(c, p, cardKeys...)
			ui.Text(c, mainLabel(cc)).FontSize(12).TextColor(p.Muted).SingleLine().Shrink(1)
			m.iconButton(c, p, "close", "Dismiss", false, m.dismissCard)
		})
		switch {
		case cc.Color != nil:
			m.colorCard(c, p, cc)
		case cc.Time != nil:
			m.timeCard(c, p, cc)
		case cc.JSON != nil:
			m.jsonCard(c, p, cc)
		}
	})
}

func badge(c *ui.Context, p kit.Palette, text string) {
	ui.Text(c, text).FontSize(12).FontWeight(600).Padding(0, 6).Radius(6).Background(p.Selected).TextColor(p.SelectedInk).SingleLine()
}

// iconButton is a small button that shows a glyph and is named for
// assistive technology. The search field keeps the keyboard after a click.
func (m *Model) iconButton(c *ui.Context, p kit.Palette, glyphName, label string, bordered bool, run func()) {
	b := ui.ButtonBase(c).Padding(4).Radius(6).Label(label)
	if bordered {
		b.Border(1, p.Border)
	}
	if b.Hovered() {
		b.Background(p.Hover)
	}
	b.Children(func() {
		ui.Icon(c, cardGlyphs[glyphName]).FontSize(12).TextColor(p.Muted)
	}).OnClick(func() {
		run()
		m.focusInput()
	})
}

// cardButton is one of the JSON card's buttons; the main one is filled.
func (m *Model) cardButton(c *ui.Context, p kit.Palette, label string, main bool, run func()) {
	b := ui.ButtonBase(c).Height(28).Padding(0, 10).Radius(6).Label(label)
	text, weight := p.Ink, 400
	switch {
	case main:
		b.Background(p.Accent)
		text, weight = p.AccentIn, 600
	case b.Hovered():
		b.Background(p.Hover).Border(1, p.Border)
	default:
		b.Background(p.Paper).Border(1, p.Border)
	}
	b.Children(func() {
		ui.Text(c, label).FontSize(12).FontWeight(weight).TextColor(text).SingleLine()
	}).OnClick(func() {
		run()
		m.focusInput()
	})
}

// cardRows shows labeled values, each with a button that copies it by the
// action of its index.
func (m *Model) cardRows(c *ui.Context, p kit.Palette, cc *ClipCard) {
	ui.Column(c).Grow(1).Shrink(1).Gap(2).Justify(ui.SpaceBetween).Children(func() {
		for i, row := range cc.Rows {
			ui.Row(c).Height(24).Gap(6).AlignItems(ui.Center).Children(func() {
				ui.Text(c, row.Label).Width(40).FontSize(12).FontWeight(600).TextColor(p.Muted).SingleLine()
				ui.Text(c, row.Value).Font(monoFont).FontSize(12).TextColor(p.Ink).SingleLine().Grow(1).Shrink(1)
				if i < len(cc.Actions) {
					m.iconButton(c, p, "copy", "Copy "+row.Label, true, func() { m.start(cc.Actions[i], "", "") })
				}
			})
		}
	})
}

func (m *Model) colorCard(c *ui.Context, p kit.Palette, cc *ClipCard) {
	col := cc.Color
	ui.Row(c).Gap(12).AlignItems(ui.Stretch).Children(func() {
		ui.Box(c).Width(56).Radius(6).Border(1, p.Border).Background(ui.Hex(col.Hex))
		m.cardRows(c, p, cc)
	})
	ui.Row(c).Gap(8).Children(func() {
		// Fixed white and black: they show the color on each, in either
		// theme.
		contrastBox(c, "white", ui.Hex("#ffffff"), ui.Hex("#302a24"), p.Rule, col.Hex, col.OnWhite)
		contrastBox(c, "black", ui.Hex("#1c1915"), ui.Hex("#f4eddf"), p.Rule, col.Hex, col.OnBlack)
	})
}

func contrastBox(c *ui.Context, on string, background, ink, rule ui.Color, hex string, k Contrast) {
	ui.Row(c).Grow(1).Shrink(1).Basis(0).Gap(6).AlignItems(ui.Center).Padding(4, 6).Radius(6).Border(1, rule).
		Background(background).Clip().Role(ui.RoleGroup).Label("Contrast on " + on).Children(func() {
		ui.Text(c, "Aa").FontWeight(700).TextColor(ui.Hex(hex)).Shrink(0)
		ui.Text(c, k.Ratio+" on "+on).FontSize(12).TextColor(ink).SingleLine().Grow(1).Shrink(1)
		ui.Text(c, k.Grade).FontSize(12).FontWeight(700).TextColor(ink).SingleLine().Shrink(0)
	})
}

func (m *Model) timeCard(c *ui.Context, p kit.Palette, cc *ClipCard) {
	t := cc.Time
	unit := "seconds"
	if t.Milliseconds {
		unit = "milliseconds"
	}
	ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
		ui.Text(c, t.Raw).Font(monoFont).FontWeight(500).TextColor(p.Ink).SingleLine()
		badge(c, p, unit)
		ui.Box(c).Grow(1)
		ui.Text(c, t.Relative).FontWeight(600).TextColor(p.Accent).SingleLine()
	})
	m.cardRows(c, p, cc)
}

func (m *Model) jsonCard(c *ui.Context, p kit.Palette, cc *ClipCard) {
	j := cc.JSON
	ui.Text(c, jsonSummary(j)).FontSize(12).TextColor(p.Muted).SingleLine()
	if len(j.Keys) > 0 {
		ui.Row(c).Gap(4).Clip().Role(ui.RoleList).Label("Keys").Children(func() {
			for _, k := range j.Keys {
				ui.RichText(c,
					ui.Span{Text: k.Name, Color: p.Ink},
					ui.Span{Text: k.Hint, Color: p.Muted},
				).Font(monoFont).FontSize(12).SingleLine().Shrink(0).Padding(1, 6).Radius(6).
					Border(1, p.Edge).Background(p.Paper).Role(ui.RoleListItem)
			}
		})
	}
	ui.Text(c, j.Minified).Font(monoFont).FontSize(12).TextColor(p.Ink).SingleLine().
		Padding(5, 8).Radius(6).Border(1, p.Edge).Background(p.Paper)
	ui.Row(c).Gap(6).Children(func() {
		m.cardButton(c, p, "View Full", true, func() { m.openJSON(cc) })
		for _, ra := range cc.Actions {
			m.cardButton(c, p, strings.Replace(ra.Label, " JSON", "", 1), false, func() { m.start(ra, "", "") })
		}
	})
}

// jsonView shows the whole JSON in place of the pane, with line numbers.
// Escape or Back closes it.
func (m *Model) jsonView(c *ui.Context, p kit.Palette, cc *ClipCard) {
	j := cc.JSON
	ui.Column(c).Grow(9).Background(p.Paper).BorderWidth(0, 0, 0, 1).BorderColor(p.Rule).Clip().
		Role(ui.RoleDialog).Label("Clipboard JSON").Children(func() {
		ui.Row(c).Padding(10, 12).Gap(8).AlignItems(ui.Center).BorderWidth(0, 0, 1, 0).BorderColor(p.Rule).Children(func() {
			m.iconButton(c, p, "back", "Back to widgets", true, func() { m.jsonFor = "" })
			ui.Column(c).Grow(1).Shrink(1).Children(func() {
				ui.Text(c, "Clipboard JSON").FontWeight(700).TextColor(p.Ink).SingleLine()
				ui.Text(c, jsonSummary(j)).FontSize(12).TextColor(p.Muted).SingleLine()
			})
			if pretty, ok := prettyAction(cc); ok {
				m.cardButton(c, p, "Copy Pretty", false, func() { m.start(pretty, "", "") })
			}
			keyCaps(c, p, "esc")
		})
		rows := len(j.Lines)
		if j.MoreLines > 0 {
			rows++
		}
		ui.List(c, &m.jsonList, rows, func(i int) {
			if i == len(j.Lines) {
				note := formatCount(j.MoreLines) + " more lines."
				if _, ok := prettyAction(cc); ok {
					note += " Copy Pretty has all of them."
				}
				ui.Text(c, note).
					FontSize(12).TextColor(p.Muted).Padding(8, 12, 8, 48)
				return
			}
			// A click leaves the keyboard with the search field.
			jsonLine(c, p, i+1, j.Lines[i]).OnClick(m.focusInput)
		}).Grow(1).Padding(8, 12, 8, 0).Label("Lines")
	})
}

func jsonLine(c *ui.Context, p kit.Palette, number int, line JSONLine) ui.Element {
	return ui.Row(c).Gap(12).AlignItems(ui.Start).Children(func() {
		ui.Text(c, strconv.Itoa(number)).Font(monoFont).FontSize(12).TextColor(p.Muted).Opacity(0.7).Width(36).TextAlign(ui.End).SingleLine()
		spans := make([]ui.Span, 0, len(line.Tokens)+1)
		if line.Indent > 0 {
			spans = append(spans, ui.Span{Text: strings.Repeat("  ", line.Indent)})
		}
		for _, t := range line.Tokens {
			spans = append(spans, ui.Span{Text: t.Text, Color: tokenColor(p, t.Kind)})
		}
		ui.RichText(c, spans...).Font(monoFont).FontSize(12).PreserveSpaces().Grow(1).Shrink(1)
	})
}

func tokenColor(p kit.Palette, kind JSONTokenKind) ui.Color {
	switch kind {
	case JSONKeyToken:
		return p.Accent
	case JSONString:
		return p.CodeString
	case JSONNumber:
		return p.CodeNumber
	case JSONLiteral:
		return p.CodeLiteral
	}
	return p.Muted
}

// prettyAction is the card's Copy Pretty JSON, which JSON too deep to
// copy pretty lacks.
func prettyAction(cc *ClipCard) (core.ResultAction, bool) {
	for _, ra := range cc.Actions {
		if ra.Label == "Copy Pretty JSON" {
			return ra, true
		}
	}
	return core.ResultAction{}, false
}

// formatCount writes n with thousands separators, as 1,002.
func formatCount(n int) string {
	s := strconv.Itoa(n)
	for i := len(s) - 3; i > 0; i -= 3 {
		s = s[:i] + "," + s[i:]
	}
	return s
}
