package store

import (
	"errors"
	"fmt"

	"github.com/egoist/mygo/plugins/sqlite"
)

// message is an error written for the user.
type message string

func (m message) Error() string { return string(m) }

// fileError is a failure to make the data folder. It names no step: files
// fail for many causes.
type fileError struct{ err error }

func (e fileError) Error() string { return fmt.Sprintf("Could not read or write a file: %v.", e.err) }
func (e fileError) Unwrap() error { return e.err }

// DataError is a failed read or write of tinydash.db. Its text ends with
// the step that fits the cause; only the store uses SQLite, so a step can
// name its file.
type DataError struct{ Err error }

func dataError(err error) error {
	if err == nil {
		return nil
	}
	return &DataError{Err: err}
}

func (e *DataError) Error() string {
	return fmt.Sprintf("Could not read or save data: %v. %s", e.Err, nextStep(e.Err))
}

func (e *DataError) Unwrap() error { return e.Err }

// SQLite's primary result codes.
const (
	codePerm     = 3
	codeBusy     = 5
	codeLocked   = 6
	codeReadOnly = 8
	codeCorrupt  = 11
	codeFull     = 13
	codeCantOpen = 14
	codeNotADB   = 26
)

func nextStep(err error) string {
	var failure *sqlite.Error
	if !errors.As(err, &failure) {
		return "Try again, or restart TinyDash."
	}
	switch failure.Code {
	case codeFull:
		return "Free some disk space, then try again."
	case codeCorrupt, codeNotADB:
		return "The saved data is damaged. Quit TinyDash, then move tinydash.db out of its data folder to start with new data."
	case codeBusy, codeLocked:
		return "Close other programs that use tinydash.db, then try again."
	case codeReadOnly, codePerm, codeCantOpen:
		return "Check that you can write to TinyDash's data folder."
	}
	return "Try again, or restart TinyDash."
}
