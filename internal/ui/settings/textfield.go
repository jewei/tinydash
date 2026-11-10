package settings

import (
	"strings"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// textField is text that saves when the field loses the focus or Enter is
// pressed. It shows the saved value while it is not edited.
type textField struct {
	draft   string
	editing bool
}

// text is a row with a textField of at most maxLength characters.
func (m *Model) text(c *ui.Context, p kit.Palette, f *textField, label, name, placeholder, value string, maxLength int, set func(*settings.Settings, string)) {
	if !f.editing {
		f.draft = value
	}
	row(c, p, label, "", func() {
		in := ui.TextInput(c, &f.draft).Label(name).Placeholder(placeholder).MaxLength(maxLength).Width(200)
		in.OnTextKey(func(k ui.TextKey) bool {
			if k.Key != ui.KeyEnter || k.Mods != 0 {
				return false
			}
			m.commitText(f, value, set)
			return true
		})
		if in.Changed() {
			f.editing = true
		}
		if f.editing && !in.Focused() {
			m.commitText(f, value, set)
		}
	})
}

// commitText saves the draft without its surrounding spaces, when it
// changed.
func (m *Model) commitText(f *textField, value string, set func(*settings.Settings, string)) {
	f.editing = false
	if text := strings.TrimSpace(f.draft); text != value {
		m.save(func(s *settings.Settings) { set(s, text) }, nil)
	}
}
