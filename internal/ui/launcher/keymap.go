package launcher

import "github.com/egoist/mygo/ui"

// binding maps a key to a model method. run gets whether the key came as
// an auto-repeat.
type binding struct {
	mods ui.Modifiers
	key  ui.Key
	// when, if set, says whether the binding takes the key now. Where it
	// does not, the key keeps its usual meaning in the search field, as
	// Mod+Backspace deleting a word.
	when func(*Model) bool
	run  func(m *Model, repeat bool)
}

// keymap is every key of the launcher, with ui.Cmd as Command on macOS and
// Control elsewhere. The menu and the dialogs take their own keys.
var keymap = append([]binding{
	{0, ui.KeyDown, nil, held(func(m *Model) { m.move(1) })},
	{0, ui.KeyUp, nil, (*Model).up},
	{0, ui.KeyEnter, nil, once((*Model).runSelected)},
	{ui.Cmd, ui.KeyEnter, nil, once((*Model).runSecondary)},
	{0, ui.KeyTab, nil, once(func(m *Model) { m.moveCategory(1) })},
	{ui.Shift, ui.KeyTab, nil, once(func(m *Model) { m.moveCategory(-1) })},
	{ui.Cmd, ui.KeyK, nil, once((*Model).openMenu)},
	{ui.Cmd, ui.KeyBackspace, (*Model).canDelete, once((*Model).deleteSelected)},
	{ui.Cmd, ui.KeyComma, nil, once((*Model).openSettings)},
	{ui.Cmd, ui.KeyP, (*Model).hasFocusTimer, once((*Model).runFocusTimer)},
	{ui.Cmd, ui.KeyJ, nil, once((*Model).focusNote)},
	{0, ui.KeyEscape, nil, once((*Model).escape)},
}, rowKeys()...)

// noteKeymap is every key of the launcher while the note has the keyboard.
var noteKeymap = []binding{
	{ui.Cmd, ui.KeyJ, nil, once((*Model).leaveNote)},
	{0, ui.KeyEscape, nil, once((*Model).leaveNote)},
	// Tab would move the focus to a button, where typing reaches nothing.
	{0, ui.KeyTab, nil, once((*Model).leaveNote)},
	{ui.Shift, ui.KeyTab, nil, once((*Model).leaveNote)},
}

// rowKeys run the main action of rows 1 to 9 with Mod and the digit.
func rowKeys() []binding {
	var out []binding
	for row := range 9 {
		out = append(out, binding{ui.Cmd, ui.Key1 + ui.Key(row), nil, once(func(m *Model) { m.runRow(row, 0) })})
	}
	return out
}

// once acts on a press only: holding a key never repeats an action.
func once(f func(*Model)) func(*Model, bool) {
	return func(m *Model, repeat bool) {
		if !repeat {
			f(m)
		}
	}
}

// held acts on every repeat too, as a held arrow keeps moving.
func held(f func(*Model)) func(*Model, bool) {
	return func(m *Model, _ bool) { f(m) }
}

// press runs the binding of a key and reports whether one took it.
func (m *Model) press(mods ui.Modifiers, key ui.Key, repeat bool) bool {
	for _, b := range keymap {
		if b.mods == mods && b.key == key && (b.when == nil || b.when(m)) {
			b.run(m, repeat)
			return true
		}
	}
	return false
}
