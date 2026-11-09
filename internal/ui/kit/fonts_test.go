package kit

import (
	"image"
	"testing"

	"github.com/egoist/mygo/ui"
)

// draw renders a line of text in a family and weight, as the views set it.
func draw(family string, weight int) image.Image {
	tt := ui.NewTester(func(c *ui.Context) {
		ui.Column(c).Fill().Background(ui.Hex("#ffffff")).Children(func() {
			ui.Text(c, "Hamburgefonstiv 0123").Font(family).FontSize(30).FontWeight(weight).TextColor(ui.Hex("#000000"))
		})
	}, 500, 80)
	return tt.Image()
}

func same(a, b image.Image) bool {
	if a.Bounds() != b.Bounds() {
		return false
	}
	r := a.Bounds()
	for y := r.Min.Y; y < r.Max.Y; y++ {
		for x := r.Min.X; x < r.Max.X; x++ {
			if a.At(x, y) != b.At(x, y) {
				return false
			}
		}
	}
	return true
}

// The text engine of each OS must find the embedded fonts by family, and
// draw Figtree's weights from its one variable font.
func TestRegisterFonts(t *testing.T) {
	system := draw("", 400)
	if err := RegisterFonts(); err != nil {
		t.Fatal(err)
	}
	if err := RegisterFonts(); err != nil {
		t.Fatalf("second call: %v", err)
	}
	regular := draw(TextFont, 400)
	if same(regular, system) {
		t.Errorf("%s draws as the system's font", TextFont)
	}
	for _, weight := range []int{600, 700} {
		if same(draw(TextFont, weight), regular) {
			t.Errorf("%s %d draws as %s 400", TextFont, weight, TextFont)
		}
	}
	if display := draw(DisplayFont, 400); same(display, regular) || same(display, system) {
		t.Errorf("%s draws as another font", DisplayFont)
	}
}
