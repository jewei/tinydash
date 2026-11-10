package engine

import (
	"strings"
	"testing"
)

func TestSavesANoteUpToTheLimitWithoutAnnouncingIt(t *testing.T) {
	e, changes := start(t)
	if note, err := e.Note(); note != "" || err != nil {
		t.Fatalf("before a save: %q %v", note, err)
	}
	// The limit counts characters, not bytes: é takes two.
	longest := strings.Repeat("é", MaxNoteChars)
	if err := <-e.SaveNote(longest); err != nil {
		t.Fatal(err)
	}
	if err := <-e.SaveNote(longest + "!"); err == nil || err.Error() != "The note holds up to 10,000 characters. Shorten it to save it." {
		t.Errorf("one character over: %v", err)
	}
	if note, err := e.Note(); note != longest || err != nil {
		t.Errorf("a refused note replaced the saved one: %d characters, %v", len([]rune(note)), err)
	}
	// The note is in no snapshot: a save refreshes no view, which would
	// load the widgets again on every pause in typing.
	if n := changes.Load(); n != 0 {
		t.Errorf("%d announcements", n)
	}
}

// Saves land in the order they were sent, so the last text typed is the
// one kept.
func TestTheLastNoteSentIsTheOneKept(t *testing.T) {
	e, _ := start(t)
	var waits []<-chan error
	for _, text := range []string{"a", "ab", "abc"} {
		waits = append(waits, e.SaveNote(text))
	}
	for _, done := range waits {
		if err := <-done; err != nil {
			t.Fatal(err)
		}
	}
	if note, _ := e.Note(); note != "abc" {
		t.Errorf("kept %q", note)
	}
}
