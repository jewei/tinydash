package launcher

import "github.com/egoist/mygo/ui"

// palette is the warm Canvas palette of src/styles/tokens.css.
type palette struct {
	paper, field, hover           ui.Color
	selected, selectedInk, edge   ui.Color
	ink, muted, rule, border      ui.Color
	accent, accentHover, accentIn ui.Color
	danger, dangerInk, notice     ui.Color
	shadow, backdrop              ui.Color
}

var light = palette{
	paper: ui.Hex("#fcf8f2"), field: ui.Hex("#ebe5da"), hover: ui.Hex("#efe8dd"),
	selected: ui.Hex("#fbe4d4"), selectedInk: ui.Hex("#763f21"), edge: ui.Hex("#e8bda0"),
	ink: ui.Hex("#302a24"), muted: ui.Hex("#6f6458"), rule: ui.Hex("#e4dbce"), border: ui.Hex("#d7ccbc"),
	accent: ui.Hex("#9c4e28"), accentHover: ui.Hex("#824020"), accentIn: ui.Hex("#fff8f1"),
	danger: ui.Hex("#a33b2c"), dangerInk: ui.Hex("#fff8f1"), notice: ui.Hex("#fff2eb"),
	shadow: ui.Hex("#302a2433"), backdrop: ui.Hex("#302a2459"),
}

var dark = palette{
	paper: ui.Hex("#302d27"), field: ui.Hex("#403b32"), hover: ui.Hex("#3d3830"),
	selected: ui.Hex("#563b29"), selectedInk: ui.Hex("#ffdabc"), edge: ui.Hex("#8d6444"),
	ink: ui.Hex("#f4eddf"), muted: ui.Hex("#c3b8a8"), rule: ui.Hex("#494236"), border: ui.Hex("#5f5545"),
	accent: ui.Hex("#efb181"), accentHover: ui.Hex("#ffd0aa"), accentIn: ui.Hex("#382819"),
	danger: ui.Hex("#ffb4a1"), dangerInk: ui.Hex("#3a1610"), notice: ui.Hex("#402310"),
	shadow: ui.Hex("#0c0b0a80"), backdrop: ui.Hex("#0c0b0a99"),
}

// themed sets the frame's theme to the palette that matches the desktop's
// appearance, keeping the desktop's other settings (text size, contrast),
// and returns the palette for the view's own elements.
func themed(c *ui.Context) palette {
	t := *c.Theme()
	p := light
	if t.Dark {
		p = dark
	}
	t.Background, t.Surface, t.SurfaceHover = p.paper, p.field, p.hover
	t.Border, t.Text, t.TextMuted = p.border, p.ink, p.muted
	t.Accent, t.AccentHover, t.AccentPressed, t.AccentText = p.accent, p.accentHover, p.accentHover, p.accentIn
	t.InactiveAccent = p.accent
	t.Danger, t.Focus, t.Selection = p.danger, p.accent, p.selected
	t.Radius = 10
	c.SetTheme(&t)
	return p
}
