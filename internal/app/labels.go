package app

import (
	"bytes"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"runtime"
)

// templateTrayIcon: macOS tints a black-and-transparent menu bar icon to
// match the bar; elsewhere the icon is drawn in color.
const templateTrayIcon = runtime.GOOS == "darwin"

// trayIcon draws a magnifying glass at 32 pixels.
func trayIcon() []byte {
	img := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	ink := color.NRGBA{R: 47, G: 111, B: 94, A: 255}
	if templateTrayIcon {
		ink = color.NRGBA{A: 255}
	}
	for y := range 32 {
		for x := range 32 {
			dx, dy := float64(x)-13, float64(y)-13
			if r := dx*dx + dy*dy; r >= 7*7 && r <= 10*10 {
				img.Set(x, y, ink)
			}
		}
	}
	for i := range 8 {
		draw.Draw(img, image.Rect(20+i, 20+i, 23+i, 23+i), image.NewUniform(ink), image.Point{}, draw.Src)
	}
	var b bytes.Buffer
	_ = png.Encode(&b, img)
	return b.Bytes()
}
