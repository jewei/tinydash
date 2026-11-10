package launcher

import (
	"errors"
	"slices"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// showNote starts a launcher whose pane holds the note text, and moves the
// keyboard to it with Mod+J.
func showNote(t *testing.T, f *fake, text string) (*Model, *ui.Tester) {
	t.Helper()
	f.widgets = Widgets{Clocks: true, Note: &Note{Text: text}}
	m, tt := showLauncher(t, f)
	tt.Key(ui.Cmd, ui.KeyJ)
	if !tt.Focused("Notepad") {
		t.Fatalf("Mod+J left the note without the keyboard; texts %q", tt.Texts())
	}
	return m, tt
}

func noteStatus(tt *ui.Tester) string {
	info, _ := tt.AccessByRole(ui.RoleStatus, "")
	return info.Name
}

func TestTypesInTheNoteAndSavesItAfterAPause(t *testing.T) {
	f := newFake(always(safari))
	m, tt := showNote(t, f, "Buy milk")
	if m.note.text != "Buy milk" || noteStatus(tt) != "2 words · Saved" {
		t.Fatalf("note %q, status %q", m.note.text, noteStatus(tt))
	}
	tt.Type(" and eggs")
	if m.note.text != "Buy milk and eggs" || m.query != "" || noteStatus(tt) != "4 words · Saving…" {
		t.Fatalf("note %q, query %q, status %q", m.note.text, m.query, noteStatus(tt))
	}
	tt.Advance(saveDelay / 2)
	tt.Type("!")
	tt.Advance(saveDelay - time.Millisecond)
	if len(f.notes) != 0 {
		t.Fatalf("saved %q before typing paused", f.notes)
	}
	tt.Advance(time.Millisecond)
	if !slices.Equal(f.notes, []string{"Buy milk and eggs!"}) || noteStatus(tt) != "4 words · Saved" {
		t.Errorf("saved %q, status %q", f.notes, noteStatus(tt))
	}
	if f.widgetLoads != 1 {
		t.Errorf("loaded the widgets %d times", f.widgetLoads)
	}
}

// Keys in the note type there, as in the Rust app: Enter breaks the line,
// and Mod+K, Mod+1, and Mod+, leave the results alone.
func TestKeysInTheNoteTypeThere(t *testing.T) {
	f := newFake(always(safari))
	m, tt := showNote(t, f, "")
	tt.Type("one")
	tt.Key(0, ui.KeyEnter)
	tt.Type("two")
	tt.Key(ui.Cmd, ui.KeyK)
	tt.Key(ui.Cmd, ui.Key1)
	tt.Key(ui.Cmd, ui.KeyComma)
	if m.note.text != "one\ntwo" {
		t.Errorf("note %q", m.note.text)
	}
	if len(f.runs) != 0 || m.menu != nil || m.category != core.CategoryAll {
		t.Errorf("keys in the note acted on the launcher: runs %+v, menu %v, tab %s", f.runs, m.menu != nil, m.category)
	}
}

// Escape and Mod+J leave the note for the search field and save it at once;
// neither hides the launcher.
func TestEscapeAndModJLeaveTheNoteAndSaveIt(t *testing.T) {
	for _, key := range []struct {
		mods ui.Modifiers
		key  ui.Key
	}{{0, ui.KeyEscape}, {ui.Cmd, ui.KeyJ}} {
		f := newFake(always(safari))
		_, tt := showNote(t, f, "")
		tt.Type("draft")
		tt.Key(key.mods, key.key)
		if !tt.Focused("Search") || f.hidden != 0 {
			t.Errorf("key %v: search focused %v, hidden %d", key.key, tt.Focused("Search"), f.hidden)
		}
		if !slices.Equal(f.notes, []string{"draft"}) {
			t.Errorf("key %v: saved %q", key.key, f.notes)
		}
		tt.Advance(saveDelay)
		if len(f.notes) != 1 {
			t.Errorf("key %v: saved again %q", key.key, f.notes)
		}
		tt.Type("saf")
		if !tt.HasText("Safari") {
			t.Errorf("key %v: typing after leaving the note does not search; texts %q", key.key, tt.Texts())
		}
	}
}

// A click on the results leaves the note, which saves it.
func TestLeavingTheNoteByAClickSavesIt(t *testing.T) {
	f := newFake(always())
	_, tt := showNote(t, f, "")
	tt.Type("draft")
	click(t, tt, "What do you need?")
	if !slices.Equal(f.notes, []string{"draft"}) {
		t.Errorf("saved %q; search focused %v, note focused %v", f.notes, tt.Focused("Search"), tt.Focused("Notepad"))
	}
}

// Mod+J goes to the search field where the note does not show: after
// typing, on another tab, or with the Notepad off.
func TestModJStaysInTheSearchFieldWithoutTheNote(t *testing.T) {
	f := newFake(always(safari))
	f.widgets = Widgets{Clocks: true}
	m, tt := showLauncher(t, f)
	tt.Key(ui.Cmd, ui.KeyJ)
	if !tt.Focused("Search") {
		t.Error("Mod+J with the Notepad off took the keyboard from the search field")
	}
	f.widgets.Note = &Note{Text: "note"}
	m.Show("")
	tt.Frame()
	tt.Type("saf")
	tt.Key(ui.Cmd, ui.KeyJ)
	tt.Type("a")
	if !tt.Focused("Search") || m.query != "safa" {
		t.Errorf("after typing: search focused %v, query %q", tt.Focused("Search"), m.query)
	}
}

// A note that cannot save says why, keeps its text, and tries again as the
// user leaves it. One that cannot save as a search starts is not lost: it
// shows again when the pane comes back, and saves then.
func TestANoteThatCannotSaveIsKeptAndTriedAgain(t *testing.T) {
	f := newFake(always(safari))
	f.noteErr = func(string) error { return errors.New("The disk is full.") }
	m, tt := showNote(t, f, "")
	tt.Type("keep me")
	tt.Advance(saveDelay)
	if noteStatus(tt) != "2 words · Not saved" || !tt.HasText("The disk is full.") {
		t.Fatalf("status %q; texts %q", noteStatus(tt), tt.Texts())
	}
	tt.Key(0, ui.KeyEscape)
	if len(f.notes) != 2 {
		t.Fatalf("leaving the note did not try again: %q", f.notes)
	}

	tt.Type("saf")
	if hasPane(tt) {
		t.Fatal("the pane shows beside a search")
	}
	// Each widget load reads the saved note, which still has the old text.
	f.widgets.Note = &Note{Text: ""}
	m.Stale()
	f.noteErr = nil
	for range 3 {
		tt.Key(0, ui.KeyBackspace)
	}
	if m.note.text != "keep me" || f.notes[len(f.notes)-1] != "keep me" || noteStatus(tt) != "2 words · Saved" {
		t.Errorf("note %q, saved %q, status %q", m.note.text, f.notes, noteStatus(tt))
	}
}

// Hiding the launcher saves the note at once, and a note that could not
// save then saves as the launcher shows again.
func TestHidingSavesTheNoteAndShowingTriesAgain(t *testing.T) {
	f := newFake(always())
	f.noteErr = func(string) error { return errors.New("The database is locked.") }
	m, tt := showNote(t, f, "")
	tt.Type("hide me")
	m.Hidden()
	if !slices.Equal(f.notes, []string{"hide me"}) {
		t.Fatalf("saved %q", f.notes)
	}
	f.noteErr = nil
	m.Show("")
	tt.Frame()
	if !slices.Equal(f.notes, []string{"hide me", "hide me"}) || noteStatus(tt) != "2 words · Saved" {
		t.Errorf("saved %q, status %q", f.notes, noteStatus(tt))
	}
	if !tt.Focused("Search") {
		t.Error("the launcher opened in the note")
	}
}

// Turning the Notepad off while a save fails keeps the text, which shows
// and saves when it is turned on again.
func TestTurningTheNotepadOffKeepsTextNotSaved(t *testing.T) {
	f := newFake(always())
	f.noteErr = func(string) error { return errors.New("The disk is full.") }
	m, tt := showNote(t, f, "")
	tt.Type("later")
	tt.Key(0, ui.KeyEscape)
	f.widgets = Widgets{Clocks: true}
	m.Stale()
	tt.Frame()
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Notepad"); ok {
		t.Fatal("the note shows while off")
	}
	f.noteErr = nil
	f.widgets.Note = &Note{Text: ""}
	m.Stale()
	tt.Frame()
	if m.note.text != "later" || f.notes[len(f.notes)-1] != "later" {
		t.Errorf("note %q, saved %q", m.note.text, f.notes)
	}

	// A saved note loads again when turned on, as another copy of TinyDash
	// may have changed it.
	f.widgets = Widgets{Clocks: true}
	m.Stale()
	tt.Frame()
	f.widgets.Note = &Note{Text: "changed"}
	m.Stale()
	tt.Frame()
	if m.note.text != "changed" {
		t.Errorf("note %q", m.note.text)
	}
}

// Every change of data loads the widgets again, and a load may have read
// the note before a save landed. The note keeps the text typed.
func TestALoadNeverReplacesTheTextTyped(t *testing.T) {
	f := newFake(always())
	m, tt := showNote(t, f, "old")
	tt.Key(ui.Cmd, ui.KeyA)
	tt.Type("new")
	tt.Advance(saveDelay)
	m.Stale()
	tt.Frame()
	if m.note.text != "new" {
		t.Errorf("note %q", m.note.text)
	}
}

// One save runs at a time, so an older text never lands last. Text typed
// while a save runs saves when it ends, also when it failed.
func TestSavesOneAtATimeAndSendsTheNewestTextNext(t *testing.T) {
	f := newFake(always())
	f.holdNotes = true
	f.noteErr = func(text string) error {
		if text == "a" {
			return errors.New("The disk is full.")
		}
		return nil
	}
	m, tt := showNote(t, f, "")
	tt.Type("a")
	tt.Advance(saveDelay)
	tt.Type("b")
	tt.Advance(saveDelay)
	m.Hidden()
	if !slices.Equal(f.notes, []string{"a"}) || noteStatus(tt) != "1 word · Saving…" {
		t.Fatalf("sent %q while a save runs, status %q", f.notes, noteStatus(tt))
	}
	f.notesWaiting[0]()
	if !slices.Equal(f.notes, []string{"a", "ab"}) {
		t.Fatalf("after the first save ended: sent %q", f.notes)
	}
	f.notesWaiting[1]()
	tt.Frame()
	if noteStatus(tt) != "1 word · Saved" || tt.HasText("The disk is full.") || len(f.notes) != 2 {
		t.Errorf("sent %q, status %q; texts %q", f.notes, noteStatus(tt), tt.Texts())
	}
}

// Tab would move the focus to a button, where typing reaches nothing: in
// the note it goes back to the search field.
func TestTabInTheNoteGoesBackToTheSearchField(t *testing.T) {
	for _, mods := range []ui.Modifiers{0, ui.Shift} {
		f := newFake(always(safari))
		m, tt := showNote(t, f, "")
		tt.Type("x")
		tt.Key(mods, ui.KeyTab)
		tt.Type("saf")
		if !tt.Focused("Search") || m.query != "saf" || m.note.text != "x" || m.category != core.CategoryAll {
			t.Errorf("mods %v: query %q, note %q, tab %s", mods, m.query, m.note.text, m.category)
		}
		if !slices.Equal(f.notes, []string{"x"}) {
			t.Errorf("mods %v: saved %q", mods, f.notes)
		}
	}
}

// A load that cannot read the note takes its field away while the user
// types there: the search field takes the keyboard, and the text is kept.
func TestANoteThatGoesAwayWhileTypedInGivesTheKeysToTheSearchField(t *testing.T) {
	f := newFake(always(safari))
	m, tt := showNote(t, f, "")
	tt.Type("abc")
	f.widgets.Note = &Note{Err: errors.New("The database is locked.")}
	m.Stale()
	tt.Frame()
	tt.Type("saf")
	if !tt.Focused("Search") || m.query != "saf" {
		t.Errorf("search focused %v, query %q", tt.Focused("Search"), m.query)
	}
	if m.note.text != "abc" || !slices.Equal(f.notes, []string{"abc"}) {
		t.Errorf("note %q, saved %q", m.note.text, f.notes)
	}
}

// A note that cannot be read says why in its card, and the other widgets
// still show.
func TestANoteThatCannotBeReadSaysWhyBesideTheOtherWidgets(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Clocks: true, Note: &Note{Err: errors.New("The database is damaged.")}}
	_, tt := showLauncher(t, f)
	if !tt.HasText("Could not read the note. The database is damaged.") {
		t.Errorf("texts %q", tt.Texts())
	}
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Clocks"); !ok {
		t.Error("the clocks went with the note")
	}
	tt.Key(ui.Cmd, ui.KeyJ)
	if !tt.Focused("Search") {
		t.Error("Mod+J left the search field for a note that does not show")
	}
}

// The Notepad alone is a widget: the pane shows for it.
func TestTheNotepadAloneShowsThePane(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Note: &Note{}}
	_, tt := showLauncher(t, f)
	if !hasPane(tt) || noteStatus(tt) != "0 words · Saved" {
		t.Errorf("status %q; texts %q", noteStatus(tt), tt.Texts())
	}
}
