package launcher

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// weatherCard shows the weather of the city in Settings, or what keeps it
// from showing, ported from src/launcher/Weather.tsx.
func (m *Model) weatherCard(c *ui.Context, p kit.Palette, w Weather) {
	card(c, p, "Weather", func() {
		note := func(text string) ui.Element { return ui.Text(c, text).FontSize(12).TextColor(p.Muted) }
		settingsButton := func() {
			ui.Button(c, "Open Settings").OnClick(m.openSettings)
		}
		switch w.State {
		case WeatherReady:
			now := c.Now()
			ui.Row(c).Gap(12).AlignItems(ui.Center).Children(func() {
				ui.Column(c).Grow(1).Shrink(1).Gap(2).Children(func() {
					note(w.Place).SingleLine()
					ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
						temperature := p.Ink
						if w.Offline {
							temperature = p.Muted
						}
						ui.Text(c, strconv.Itoa(w.Temperature)+"°").Font(kit.DisplayFont).FontSize(34).TextColor(temperature).FontFeatures("tnum")
						ui.Column(c).Shrink(1).Children(func() {
							ui.Text(c, w.Condition).FontWeight(600).TextColor(p.Ink)
							note(highLow(w))
						})
					})
				})
				ui.Image(c, weatherIcon(p, w.Icon)).Width(36).Height(36)
			})
			if w.Offline {
				note("Offline · updated " + ago(w.UpdatedAt, now))
				// A new frame as the minute since the download turns.
				c.After(time.Minute - now.Sub(w.UpdatedAt)%time.Minute)
			} else {
				note("Updated " + w.UpdatedAt.Format("15:04"))
			}
		case WeatherLoading:
			note(w.City).SingleLine()
			note("Getting the weather…").Role(ui.RoleStatus)
		case WeatherNoCity:
			ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
				ui.Image(c, weatherIcon(p, "partlyDay")).Width(28).Height(28)
				ui.Text(c, "Set a city in Settings.").TextColor(p.Ink).Grow(1)
				settingsButton()
			})
		case WeatherNotFound:
			ui.Row(c).Gap(10).AlignItems(ui.Center).Children(func() {
				ui.Text(c, "No place is named “"+w.City+"”.").TextColor(p.Ink).Grow(1).Shrink(1)
				settingsButton()
			})
		case WeatherFailed:
			note(w.City).SingleLine()
			note("Could not get the weather. " + w.Message)
		}
	})
}

// highLow is today's high and low, and the chance of rain when known.
func highLow(w Weather) string {
	text := fmt.Sprintf("H %d° · L %d°", w.High, w.Low)
	if w.RainChance != nil {
		text += fmt.Sprintf(" · Rain %d%%", *w.RainChance)
	}
	return text
}

// ago is how long before now t was, in words: "5 min ago", "3 h ago".
func ago(t, now time.Time) string {
	minutes := max(0, int(now.Sub(t)/time.Minute))
	switch hours, days := minutes/60, minutes/(60*24); {
	case minutes < 1:
		return "just now"
	case minutes < 60:
		return strconv.Itoa(minutes) + " min ago"
	case hours < 24:
		return strconv.Itoa(hours) + " h ago"
	case days == 1:
		return "1 day ago"
	default:
		return strconv.Itoa(days) + " days ago"
	}
}

// weatherIcons are the Solid app's weather pictures on a 24-unit grid, for
// the light and the dark palette: the sun and the moon in the accent
// color, clouds filled so a sun behind them stays hidden, and rain and
// snow in green.
var weatherIcons = map[bool]map[string]*ui.SVG{
	false: weatherIconSet(kit.Light, ui.Hex("#4f7a63")),
	true:  weatherIconSet(kit.Dark, ui.Hex("#9fcfb0")),
}

// weatherIcon is the picture name in the palette p.
func weatherIcon(p kit.Palette, name string) *ui.SVG { return weatherIcons[p == kit.Dark][name] }

func weatherIconSet(p kit.Palette, drop ui.Color) map[string]*ui.SVG {
	const (
		rainCloud  = "M7 14h10.5a4 4 0 0 0 .4-7.98A6 6 0 0 0 6.3 7.2 3.5 3.5 0 0 0 7 14z"
		smallCloud = "M9 20.5h8.5a3.6 3.6 0 0 0 .3-7.18A5.2 5.2 0 0 0 8 14.3 3.1 3.1 0 0 0 9 20.5z"
	)
	cloud := func(d string) string { return `<path class="cloud" d="` + d + `"/>` }
	shapes := map[string]string{
		"clearDay": `<circle class="sun" cx="12" cy="12" r="4"/>` +
			`<path class="sun" d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4"/>`,
		"clearNight": `<path class="sun" d="M19.5 14.6A8 8 0 1 1 9.4 4.5a6.3 6.3 0 0 0 10.1 10.1z"/>` +
			`<path class="sun thin" d="M17 4v2M16 5h2"/>`,
		"partlyDay": `<circle class="sun" cx="9" cy="8.5" r="3.4"/>` +
			`<path class="sun" d="M9 2.5v1.3M3 8.5h1.3M4.8 4.3l.9.9M13.2 4.3l-.9.9"/>` + cloud(smallCloud),
		"partlyNight": `<path class="sun" d="M12.6 8.6A4.4 4.4 0 1 1 7.2 3.2a3.5 3.5 0 0 0 5.4 5.4z"/>` + cloud(smallCloud),
		"cloudy": `<path class="cloud faint" d="M4.6 12.6A3.2 3.2 0 0 1 6 6.6a4.8 4.8 0 0 1 8.9-1"/>` +
			cloud("M7 19.5h10.5a4 4 0 0 0 .4-7.98A6 6 0 0 0 6.3 12.7 3.5 3.5 0 0 0 7 19.5z"),
		"fog": cloud("M7 13.5h10.5a4 4 0 0 0 .4-7.98A6 6 0 0 0 6.3 6.7 3.5 3.5 0 0 0 7 13.5z") +
			`<path class="cloud" d="M4 17.5h16M6.5 21h11"/>`,
		"rain":    cloud(rainCloud) + `<path class="drop" d="M9 17.2l-1 3M13 17.2l-1 3M17 17.2l-1 3"/>`,
		"thunder": cloud(rainCloud) + `<path class="sun" d="M13 15.2l-2.6 4h3.4l-2 3.6"/>`,
		"snow":    cloud(rainCloud) + `<path class="drop flake" d="M8.5 17.6v.01M12 18.6v.01M15.5 17.6v.01M10.2 21.2v.01M13.8 21.2v.01"/>`,
	}
	style := strings.NewReplacer("ACCENT", hex(p.Accent), "PAPER", hex(p.Paper), "MUTED", hex(p.Muted), "DROP", hex(drop)).Replace(
		`.sun{stroke:ACCENT}.thin{stroke-width:1.4}.cloud{fill:PAPER;stroke:MUTED}.faint{fill:none;opacity:.65}.drop{stroke:DROP}.flake{stroke-width:2.4}`)
	out := make(map[string]*ui.SVG, len(shapes))
	for name, shape := range shapes {
		out[name] = ui.MustParseSVG([]byte(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><style>` +
			style + `</style>` + shape + `</svg>`))
	}
	return out
}

func hex(c ui.Color) string { return fmt.Sprintf("#%02x%02x%02x", c.R, c.G, c.B) }
