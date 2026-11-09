package kit

import (
	_ "embed"
	"sync"

	"github.com/egoist/mygo/ui"
)

// The Solid app's fonts, from github.com/google/fonts (ofl/figtree and
// ofl/caprasimo), under the SIL Open Font License beside each file.
// Figtree is one variable font: smaller than the three weights the views
// use as separate files.
var (
	//go:embed fonts/figtree/Figtree-VariableFont_wght.ttf
	figtree []byte
	//go:embed fonts/caprasimo/Caprasimo-Regular.ttf
	caprasimo []byte
)

const (
	// TextFont is the family of all text, which Themed sets in the theme.
	TextFont = "Figtree"
	// DisplayFont is the family of the large titles and clock times that
	// the Solid app set in Caprasimo. It has one weight, regular.
	DisplayFont = "Caprasimo"
)

var registerFonts = sync.OnceValue(func() error {
	if err := ui.RegisterFont(figtree, TextFont); err != nil {
		return err
	}
	return ui.RegisterFont(caprasimo, DisplayFont)
})

// RegisterFonts adds TextFont and DisplayFont for the views to draw with.
// The app calls it at startup, before its first window draws; until then,
// text falls back to the system's font. Later calls return the first
// call's result.
func RegisterFonts() error { return registerFonts() }
