package launcher

import (
	"strconv"
	"strings"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// saveDelay is how long typing pauses before the note saves.
const saveDelay = 400 * time.Millisecond

// notepad is the Notepad widget's state, ported from Notepad in
// src/launcher/WidgetPane.tsx. A pause in typing, leaving the note, and the
// pane closing save it. A failed save leaves the text unsaved, so the next
// of those, or the pane coming back, tries again.
type notepad struct {
	// text is the note as the field shows it. Once loaded, only the field
	// changes it: a load that read tinydash.db before a save landed would
	// bring back older text.
	text   string
	loaded bool
	// unsaved: text has changes that no save has sent yet.
	unsaved bool
	// saveAt is when the pause in typing ends and the note saves, or zero.
	saveAt time.Time
	// One save runs at a time, so an older text never lands last. saving
	// is set while it runs; again asks for one more save when it ends.
	saving, again bool
	// failure is why the last save failed, until the next edit.
	failure string
	field   ui.Handle
	// focused and shown are as the last frame had them, so the view sees
	// the user leave the note and the pane close or come back.
	focused, shown bool
	// reveal is set in the frame the note gets the keyboard: the pane
	// scrolls, and the note may be below its fold.
	reveal bool
}

// takeNote takes the note of a widget load.
func (m *Model) takeNote(n *Note) {
	switch {
	case n == nil:
		// The Notepad is off. A note with nothing left to save, and no
		// save running, loads again when it is turned on. Otherwise it
		// keeps its text: a load may have read the note before the save
		// landed, and a note not saved shows and saves again then.
		if !m.note.unsaved && !m.note.saving {
			m.note.loaded = false
		}
	case n.Err == nil && !m.note.loaded:
		m.note.text, m.note.loaded = n.Text, true
	}
}

// showsNote reports whether the note's field is on screen.
func (m *Model) showsNote() bool {
	n := m.widgets.Note
	return m.showsWidgets() && n != nil && n.Err == nil
}

// focusNote moves the keyboard to the note, or to the search field when the
// note does not show (Mod+J).
func (m *Model) focusNote() {
	if m.showsNote() {
		// The full JSON view covers the note, so typing would go to a
		// hidden field.
		m.jsonFor = ""
		m.note.field.Focus()
		return
	}
	m.input.Focus()
}

// leaveNote moves the keyboard to the search field (Mod+J, Escape, Tab, or
// Shift+Tab in the note). The next frame sees the note lose it, and saves.
func (m *Model) leaveNote() { m.input.Focus() }

// saveNote sends the note to save, if it has changes no save has sent.
// While a save runs, it waits for that one to end.
func (m *Model) saveNote() {
	m.note.saveAt = time.Time{}
	if !m.note.unsaved {
		return
	}
	if m.note.saving {
		m.note.again = true
		return
	}
	m.note.unsaved, m.note.saving = false, true
	m.backend.SaveNote(m.note.text, func(err error) {
		m.note.saving = false
		if err != nil {
			m.note.unsaved = true
			m.note.failure = err.Error()
		} else {
			m.note.failure = ""
		}
		// A save asked for while this one ran has newer text to send.
		if m.note.again {
			m.note.again = false
			m.saveNote()
		}
	})
}

// noteFrame saves the note as the frame finds the user left it, the pane
// closed, or the pause in typing ended, and tries a failed save again as
// the pane comes back. inNote is whether the note has the keyboard.
func (m *Model) noteFrame(c *ui.Context, inNote bool) {
	shown := m.showsNote()
	left := m.note.focused && !inNote
	closed := m.note.shown && !shown
	back := !m.note.shown && shown
	if left || closed || back {
		m.saveNote()
	}
	// A note that closes with the keyboard, as when a load cannot read
	// it, leaves no field to type in: the search field takes the keys.
	if m.note.focused && closed && !m.overlay() {
		m.input.Focus()
	}
	if !m.note.saveAt.IsZero() {
		if wait := m.note.saveAt.Sub(c.Now()); wait > 0 {
			c.After(wait)
		} else {
			m.saveNote()
		}
	}
	m.note.reveal = inNote && !m.note.focused
	m.note.focused, m.note.shown = inNote, shown
}

// notepadCard shows the note, or why it could not be read.
func (m *Model) notepadCard(c *ui.Context, p kit.Palette, n Note) {
	if n.Err != nil {
		card(c, p, "Notepad", func() {
			ui.Text(c, "Could not read the note. "+n.Err.Error()).FontSize(12).TextColor(p.Muted)
		})
		return
	}
	note := ui.Column(c).Grow(1).Padding(10, 12).Gap(6).Radius(10).Border(1, p.Rule).Background(p.Paper).
		Role(ui.RoleGroup).Label("Notepad")
	// The pane scrolls, so nothing else stops a long note from growing it:
	// the note is at most as tall as the pane holds, and scrolls inside.
	if room := m.paneRoom - 2*panePadding; room > 0 {
		note.MaxHeight(room)
	}
	if m.note.reveal {
		note.ScrollIntoView()
	}
	note.Children(func() {
		ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
			ui.Text(c, "Notepad").FontSize(12).FontWeight(600).TextColor(p.Muted)
			keyCaps(c, p, modKey, "J")
			ui.Box(c).Grow(1)
			ui.Text(c, wordCount(m.note.text)+" · "+m.note.status()).FontSize(12).TextColor(p.Muted).
				SingleLine().Role(ui.RoleStatus)
		})
		field := ui.TextAreaBase(c, &m.note.text).Bind(&m.note.field).Grow(1).MinHeight(40).
			TextColor(p.Ink).Placeholder("Jot something down…").Label("Notepad")
		if field.Changed() {
			m.note.unsaved = true
			m.note.failure = ""
			m.note.saveAt = c.Now().Add(saveDelay)
			// A frame as the pause ends, which saves.
			c.After(saveDelay)
		}
		if m.note.failure != "" {
			ui.Text(c, m.note.failure).FontSize(12).TextColor(p.Danger).Role(ui.RoleAlert)
		}
	})
}

// status says whether the note is saved; the reason of a failure shows
// under the note.
func (n *notepad) status() string {
	switch {
	case n.failure != "":
		return "Not saved"
	case n.unsaved || n.saving:
		return "Saving…"
	}
	return "Saved"
}

func wordCount(text string) string {
	if n := len(strings.Fields(text)); n != 1 {
		return strconv.Itoa(n) + " words"
	}
	return "1 word"
}
