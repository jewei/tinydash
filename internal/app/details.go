package app

import (
	"io/fs"
	"os"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// Details loads a result's details off the main thread, since the disk and
// tinydash.db can make it wait, and decodes a copied image there too. The
// launcher keeps only the load it asked for last.
func (b *backend) Details(id core.ID, done func(launcher.Details, bool)) {
	a := b.app()
	go func() {
		d, ok := a.actions.Details(id, osLstat{})
		out := launcher.Details{Details: d}
		if ok && d.Kind == engine.DetailsImage {
			// An image that does not decode shows its row's words alone.
			out.Image, _ = ui.DecodeBitmap(d.PNG)
		}
		a.panel.Update(func() { done(out, ok) })
	}()
}

// osLstat reads the real disk for the details of apps and files.
type osLstat struct{}

func (osLstat) Lstat(path string) (fs.FileInfo, error) { return os.Lstat(path) }
