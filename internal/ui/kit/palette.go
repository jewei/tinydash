package kit

import (
	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/settings"
)

// Palette is the warm Canvas palette of src/styles/tokens.css.
type Palette struct {
	Paper, Raised, Field, Hover   ui.Color
	Selected, SelectedInk, Edge   ui.Color
	Ink, Muted, Rule, Border      ui.Color
	Accent, AccentHover, AccentIn ui.Color
	Danger, DangerInk, Notice     ui.Color
	Shadow, Backdrop              ui.Color
}

var Light = Palette{
	Paper: ui.Hex("#fcf8f2"), Raised: ui.Hex("#f5efe6"), Field: ui.Hex("#ebe5da"), Hover: ui.Hex("#efe8dd"),
	Selected: ui.Hex("#fbe4d4"), SelectedInk: ui.Hex("#763f21"), Edge: ui.Hex("#e8bda0"),
	Ink: ui.Hex("#302a24"), Muted: ui.Hex("#6f6458"), Rule: ui.Hex("#e4dbce"), Border: ui.Hex("#d7ccbc"),
	Accent: ui.Hex("#9c4e28"), AccentHover: ui.Hex("#824020"), AccentIn: ui.Hex("#fff8f1"),
	Danger: ui.Hex("#a33b2c"), DangerInk: ui.Hex("#fff8f1"), Notice: ui.Hex("#fff2eb"),
	Shadow: ui.Hex("#302a2433"), Backdrop: ui.Hex("#302a2459"),
}

var Dark = Palette{
	Paper: ui.Hex("#302d27"), Raised: ui.Hex("#37332c"), Field: ui.Hex("#403b32"), Hover: ui.Hex("#3d3830"),
	Selected: ui.Hex("#563b29"), SelectedInk: ui.Hex("#ffdabc"), Edge: ui.Hex("#8d6444"),
	Ink: ui.Hex("#f4eddf"), Muted: ui.Hex("#c3b8a8"), Rule: ui.Hex("#494236"), Border: ui.Hex("#5f5545"),
	Accent: ui.Hex("#efb181"), AccentHover: ui.Hex("#ffd0aa"), AccentIn: ui.Hex("#382819"),
	Danger: ui.Hex("#ffb4a1"), DangerInk: ui.Hex("#3a1610"), Notice: ui.Hex("#402310"),
	Shadow: ui.Hex("#0c0b0a80"), Backdrop: ui.Hex("#0c0b0a99"),
}

// Themed sets the frame's theme to the palette of the Appearance setting,
// or of the desktop's appearance for Match System, keeping the desktop's
// other settings (text size, contrast), and returns the palette for the
// view's own elements.
func Themed(c *ui.Context, theme settings.Theme) Palette {
	t := *c.Theme()
	t.Dark = theme == settings.ThemeDark || theme != settings.ThemeLight && t.Dark
	p := Light
	if t.Dark {
		p = Dark
	}
	t.Background, t.Surface, t.SurfaceHover = p.Paper, p.Field, p.Hover
	t.Border, t.Text, t.TextMuted = p.Border, p.Ink, p.Muted
	t.Accent, t.AccentHover, t.AccentPressed, t.AccentText = p.Accent, p.AccentHover, p.AccentHover, p.AccentIn
	t.InactiveAccent = p.Accent
	t.Danger, t.Focus, t.Selection = p.Danger, p.Accent, p.Selected
	t.Radius = 10
	c.SetTheme(&t)
	return p
}
