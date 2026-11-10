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
	// held: the draft was committed and is not saved yet, or its save
	// failed, so the field keeps it rather than the saved value.
	held bool
	// saving counts the saves on their way. While one is, the saved value
	// is not the last one committed, so each commit saves.
	saving int
}

// text is a row with a textField of at most maxLength characters.
func (m *Model) text(c *ui.Context, p kit.Palette, f *textField, label, name, placeholder, value string, maxLength int, set func(*settings.Settings, string)) {
	if !f.editing && !f.held {
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
			f.editing, f.held = true, false
		}
		if f.editing && !in.Focused() {
			m.commitText(f, value, set)
		}
	})
}

// commitText saves the draft without its surrounding spaces, when it
// changed. The field shows the trimmed text until the save ends; after a
// failure it keeps it, so the text is not lost.
func (m *Model) commitText(f *textField, value string, set func(*settings.Settings, string)) {
	f.editing = false
	text := strings.TrimSpace(f.draft)
	f.draft = text
	if text == value && f.saving == 0 {
		return
	}
	f.held = true
	f.saving++
	m.save(func(s *settings.Settings) { set(s, text) }, func(err error) {
		f.saving--
		if err == nil && f.draft == text {
			f.held = false
		}
	})
}
