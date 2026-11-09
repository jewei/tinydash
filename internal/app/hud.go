package app

import (
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

const (
	hudWidth, hudHeight = 160, 44
	// hudShownFor is how long a message stays before it fades.
	hudShownFor = 1200 * time.Millisecond
	hudFade     = 200 * time.Millisecond
)

// hud shows a short message that confirms a copy after the launcher hides,
// such as "Copied". Its panel never takes focus, so the app the user went
// back to keeps the keyboard, and the pointer passes through it.
type hud struct {
	window hudWindow
	// after runs f on another goroutine after d (time.AfterFunc).
	after func(d time.Duration, f func())
	// area is the work area the message goes in: that of the launcher's
	// display, not of the display the message was on last.
	area func() mygo.Rectangle

	// message, shown, and fading belong to the main thread. shown counts
	// the messages, so only the newest one fades and hides the panel.
	message string
	shown   uint64
	fading  bool
}

// hudWindow is what hud needs of its *mygo.Window.
type hudWindow interface {
	Update(fn func())
	SetPosition(x, y int)
	ShowInactive()
	SetIgnoreMouseEvents(ignore bool)
	Hide()
}

// newHUD makes the hidden panel at startup, so the first message shows at
// once.
func newHUD(launcher *mygo.Window) *hud {
	h := &hud{after: afterFunc, area: func() mygo.Rectangle {
		return mygo.Screen.DisplayMatching(launcher.Bounds()).WorkArea
	}}
	h.window = mygo.NewWindow(mygo.WindowOptions{
		Title:         "TinyDash Message",
		Panel:         true,
		Hidden:        true,
		Frameless:     true,
		Transparent:   true,
		DisableShadow: true,
		DisableResize: true,
		Width:         hudWidth,
		Height:        hudHeight,
		Content:       ui.View(h.view),
	})
	return h
}

func afterFunc(d time.Duration, f func()) { time.AfterFunc(d, f) }

// show puts text near the bottom of the launcher's display, then fades
// and hides it. Call it from any goroutine.
func (h *hud) show(text string) {
	h.window.Update(func() {
		h.shown++
		shown := h.shown
		h.message, h.fading = text, false
		h.window.SetPosition(hudSpot(h.area(), hudWidth, hudHeight))
		h.window.ShowInactive()
		h.window.SetIgnoreMouseEvents(true)
		h.after(hudShownFor, func() {
			h.window.Update(func() {
				if h.shown == shown {
					h.fading = true
				}
			})
		})
		h.after(hudShownFor+hudFade, func() {
			h.window.Update(func() {
				if h.shown == shown {
					h.window.Hide()
				}
			})
		})
	})
}

// hudSpot is the top-left corner of a message of w × h centered across
// area, with its middle four fifths down.
func hudSpot(area mygo.Rectangle, w, h int) (x, y int) {
	return area.X + (area.Width-w)/2, area.Y + area.Height*4/5 - h/2
}

// view draws the pill. It keeps one dark look in both themes, as system
// messages do.
func (h *hud) view(c *ui.Context) {
	opacity := float32(1)
	if h.fading {
		opacity = 0
	}
	ui.Row(c).Fill().Justify(ui.Center).AlignItems(ui.Center).Children(func() {
		pill := ui.Row(c).Height(36).Padding(0, 16).Gap(8).AlignItems(ui.Center).Radius(18).
			Background(ui.Hex("#24201ce6")).TextColor(ui.Hex("#f4eddf")).Role(ui.RoleStatus)
		pill.Opacity(pill.Animate("fade", opacity, hudFade)).Children(func() {
			ui.Icon(c, checkGlyph).FontSize(16)
			ui.Text(c, h.message).FontWeight(600)
		})
	})
}

var checkGlyph = ui.MustParseSVG([]byte(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5L20 7"/></svg>`))
