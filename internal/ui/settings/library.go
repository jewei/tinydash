package settings

import "slices"

// libraryEditor creates, edits, and deletes snippets or quicklinks. Each
// kind has its own, which keeps an unsaved edit, and a save that still
// runs, while another section shows.
type libraryEditor struct {
	m         *Model
	quicklink bool
	draft     LibraryItem
	// session counts the items opened in the editor, a new one included:
	// the reply to a save or delete, and its error, change only the item it
	// came from.
	session    int
	err        string
	confirming bool
	// busy is set while a save or delete runs, so a double click cannot
	// save twice.
	busy bool
	// focus is the control that takes the focus in the next frame.
	focus libraryFocus
}

type libraryFocus uint8

const (
	focusNone libraryFocus = iota
	focusName
	focusKeep
	focusDelete
)

func newEditor(m *Model, quicklink bool) *libraryEditor {
	ed := &libraryEditor{m: m, quicklink: quicklink}
	ed.edit(ed.blank())
	return ed
}

func (ed *libraryEditor) blank() LibraryItem { return LibraryItem{Quicklink: ed.quicklink} }

func (ed *libraryEditor) noun() string {
	if ed.quicklink {
		return "Quicklink"
	}
	return "Snippet"
}

// items are the saved items of the editor's kind.
func (ed *libraryEditor) items() []LibraryItem {
	var out []LibraryItem
	for _, it := range ed.m.library {
		if it.Quicklink == ed.quicklink {
			out = append(out, it)
		}
	}
	return out
}

func (ed *libraryEditor) edit(item LibraryItem) {
	ed.session++
	ed.draft = item
	ed.err = ""
	ed.confirming = false
}

// savedName is the name Delete asks with: the saved one, not an unsaved
// edit.
func (ed *libraryEditor) savedName() string {
	i := slices.IndexFunc(ed.m.library, func(it LibraryItem) bool { return it.ID == ed.draft.ID })
	if i < 0 {
		return ed.draft.Name
	}
	return ed.m.library[i].Name
}

// listChanged keeps an open item that a delete removed open as a new one,
// with its text, so Save adds it again instead of failing.
func (ed *libraryEditor) listChanged() {
	id := ed.draft.ID
	if id != 0 && !slices.ContainsFunc(ed.m.library, func(it LibraryItem) bool { return it.ID == id }) {
		ed.draft.ID = 0
		ed.confirming = false
	}
}

// begin starts a write, unless one runs, and returns the session it came
// from.
func (ed *libraryEditor) begin() (int, bool) {
	if ed.busy {
		return 0, false
	}
	ed.busy, ed.err = true, ""
	return ed.session, true
}

// end finishes a write: the list shows what the backend now holds, also
// after a failure.
func (ed *libraryEditor) end() {
	ed.busy = false
	ed.m.library = ed.m.backend.Library()
	for _, other := range ed.m.editors {
		other.listChanged()
	}
}

// save sends the draft. Typing goes on while it runs: the reply replaces
// the draft only if nothing changed, and otherwise gives a new item its
// ID. Another item opened meanwhile stays as it is.
func (ed *libraryEditor) save() {
	from, ok := ed.begin()
	if !ok {
		return
	}
	sent := ed.draft
	ed.m.backend.SaveLibraryItem(sent, func(saved LibraryItem, err error) {
		open := ed.session == from
		switch {
		case err != nil && open:
			ed.err = err.Error()
		case err != nil:
		case open && ed.draft == sent:
			ed.edit(saved)
		case open:
			ed.draft.ID = saved.ID
		}
		ed.end()
	})
}

func (ed *libraryEditor) remove() {
	id := ed.draft.ID
	if id == 0 {
		return
	}
	from, ok := ed.begin()
	if !ok {
		return
	}
	ed.m.backend.DeleteLibraryItem(id, func(err error) {
		open := ed.session == from
		switch {
		case err != nil && open:
			ed.err = err.Error()
		case err == nil && open:
			ed.edit(ed.blank())
			ed.focus = focusName
		}
		ed.end()
	})
}

func (ed *libraryEditor) askDelete() {
	ed.confirming = true
	ed.focus = focusKeep
}

func (ed *libraryEditor) keep() {
	ed.confirming = false
	ed.focus = focusDelete
}
