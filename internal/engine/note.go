package engine

import (
	"context"
	"errors"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// MaxNoteChars is the longest note the Notepad saves, in characters
// (MAX_NOTE_CHARS of the Rust app). A longer one stays in the field, not
// saved, until the user shortens it.
const MaxNoteChars = 10_000

// errNoteTooLong says MaxNoteChars in words.
var errNoteTooLong = errors.New("The note holds up to 10,000 characters. Shorten it to save it.")

// Note reads the Notepad's note. It reads tinydash.db, so it must not run on
// the main thread.
func (e *Engine) Note() (string, error) {
	return e.store.Note(context.Background())
}

// SaveNote saves the Notepad's note on the writer, so saves land in the
// order they were sent and an older text never lands last. The note is in
// no snapshot, so a save publishes nothing and refreshes no view.
func (e *Engine) SaveNote(text string) <-chan error {
	return e.Change(func(_ *search.Snapshot, db *store.Store) error {
		if utf8.RuneCountInString(text) > MaxNoteChars {
			return errNoteTooLong
		}
		if err := db.SaveNote(context.Background(), text); err != nil {
			return err
		}
		return errUnchanged
	})
}
