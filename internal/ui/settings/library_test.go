package settings

import (
	"errors"
	"testing"

	"github.com/egoist/mygo/ui"
)

// The Snippets and Quicklinks tests of src/settings/Settings.test.tsx.

func fieldValue(t *testing.T, tt *ui.Tester, name string) string {
	t.Helper()
	info, ok := tt.AccessByRole(ui.RoleTextField, name)
	if !ok {
		t.Fatalf("no field %q; texts %q", name, tt.Texts())
	}
	return info.Value
}

func hasButton(tt *ui.Tester, name string) bool {
	_, ok := tt.AccessByRole(ui.RoleButton, name)
	return ok
}

func TestCreatesASnippet(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Signature", false)
	typeInto(t, tt, "Text", "Regards, {date}", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	if len(f.saves) != 1 || f.saves[0] != (LibraryItem{Name: "Signature", Text: "Regards, {date}"}) {
		t.Fatalf("saves %+v", f.saves)
	}
	if !hasButton(tt, "Signature") || !hasButton(tt, "Delete") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestShowsWhyAnItemWasRefused(t *testing.T) {
	f := newFake()
	f.saveErr = func(LibraryItem) error { return errors.New("Enter a name of up to 100 characters.") }
	_, tt := open(t, f, quicklinks)
	press(t, tt, ui.RoleButton, "Save Quicklink")
	if info, ok := tt.AccessByRole(ui.RoleAlert, ""); !ok || info.Name != "Enter a name of up to 100 characters." {
		t.Errorf("%+v %v", info, ok)
	}
	if !f.saves[0].Quicklink {
		t.Error("a quicklink saved as a snippet")
	}
}

func TestKeepsAnUnsavedSnippetAndQuicklinkWhileAnotherSectionIsOpen(t *testing.T) {
	_, tt := open(t, newFake(), snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hello", false)
	click(t, tt, "Quicklinks")
	// Each kind has its own draft.
	if got := fieldValue(t, tt, "Name"); got != "" {
		t.Fatalf("quicklink name %q", got)
	}
	typeInto(t, tt, "Name", "Docs", false)
	click(t, tt, "General")
	click(t, tt, "Snippets")
	if fieldValue(t, tt, "Name") != "Greeting" || fieldValue(t, tt, "Text") != "Hello" {
		t.Errorf("snippet %q %q", fieldValue(t, tt, "Name"), fieldValue(t, tt, "Text"))
	}
	click(t, tt, "Quicklinks")
	if got := fieldValue(t, tt, "Name"); got != "Docs" {
		t.Errorf("quicklink name %q", got)
	}
}

func TestFinishesASnippetSaveThatRunsWhileAnotherSectionIsOpen(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hi", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	click(t, tt, "Quicklinks")
	click(t, tt, "Snippets")
	// Back while it still runs: Save cannot send it a second time.
	if save, _ := tt.AccessByRole(ui.RoleButton, "Save Snippet"); !save.Disabled {
		t.Error("Save is on while a save runs")
	}
	click(t, tt, "Quicklinks")
	f.release(t, tt)
	click(t, tt, "Snippets")
	// The saved item stays open with its ID, so the next Save updates it.
	if !hasButton(tt, "Greeting") || !hasButton(tt, "Delete") || fieldValue(t, tt, "Name") != "Greeting" {
		t.Fatalf("texts %q", tt.Texts())
	}
	press(t, tt, ui.RoleButton, "Save Snippet")
	if len(f.saves) != 2 || f.saves[1].ID != 101 {
		t.Errorf("saves %+v", f.saves)
	}
}

func TestKeepsASnippetAndShowsWhyItsSaveFailedWhileAnotherSectionWasOpen(t *testing.T) {
	f := newFake()
	f.hold = true
	f.saveErr = func(LibraryItem) error { return errors.New("The keyword is in use.") }
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hi", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	click(t, tt, "Quicklinks")
	f.release(t, tt)
	click(t, tt, "Snippets")
	if !tt.HasText("The keyword is in use.") || fieldValue(t, tt, "Name") != "Greeting" || fieldValue(t, tt, "Text") != "Hi" {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestKeepsTypingThatHappensWhileASnippetSaves(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hello", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	press(t, tt, ui.RoleTextField, "Text")
	tt.Type(" world")
	f.release(t, tt)
	if !hasButton(tt, "Delete") || fieldValue(t, tt, "Text") != "Hello world" {
		t.Errorf("text %q, texts %q", fieldValue(t, tt, "Text"), tt.Texts())
	}
}

func TestLeavesANewSnippetOpenedDuringASaveWithoutTheSavedOnesID(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hi", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	press(t, tt, ui.RoleButton, "New Snippet")
	f.release(t, tt)
	if hasButton(tt, "Delete") {
		t.Error("the new snippet took the saved one's ID")
	}
}

func TestShowsAFailedSavesErrorOnlyWithTheItemItCameFrom(t *testing.T) {
	f := newFake()
	f.hold = true
	f.saveErr = func(LibraryItem) error { return errors.New("The keyword is in use.") }
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hi", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	press(t, tt, ui.RoleButton, "New Snippet")
	f.release(t, tt)
	if tt.HasText("The keyword is in use.") {
		t.Error("another item shows the error")
	}
}

func TestClearsAnOldSaveErrorWhenALaterSaveWorks(t *testing.T) {
	f := newFake()
	calls := 0
	f.saveErr = func(LibraryItem) error {
		calls++
		if calls == 1 {
			return errors.New("The keyword is in use.")
		}
		return nil
	}
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Greeting", false)
	typeInto(t, tt, "Text", "Hi", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	if !tt.HasText("The keyword is in use.") {
		t.Fatal("no error")
	}
	press(t, tt, ui.RoleButton, "Save Snippet")
	if tt.HasText("The keyword is in use.") || !hasButton(tt, "Delete") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestSavesASnippetOnceWhenSaveIsClickedTwice(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, snippets)
	typeInto(t, tt, "Name", "Sig", false)
	typeInto(t, tt, "Text", "Regards", false)
	press(t, tt, ui.RoleButton, "Save Snippet")
	press(t, tt, ui.RoleButton, "Save Snippet")
	if len(f.saves) != 1 {
		t.Errorf("%d saves", len(f.saves))
	}
}

func TestLeavesAnItemOpenedDuringADeleteAsItIs(t *testing.T) {
	f := newFake()
	f.library = []LibraryItem{{ID: 1, Name: "Alpha", Text: "A"}, {ID: 2, Name: "Beta", Text: "B"}}
	f.hold = true
	_, tt := open(t, f, snippets)
	press(t, tt, ui.RoleButton, "Alpha")
	press(t, tt, ui.RoleButton, "Delete")
	press(t, tt, ui.RoleButton, "Delete")
	press(t, tt, ui.RoleButton, "Beta")
	press(t, tt, ui.RoleTextField, "Text")
	tt.Type(" edited")
	f.release(t, tt)
	if got := fieldValue(t, tt, "Text"); got != "B edited" {
		t.Errorf("text %q", got)
	}
	if len(f.deletes) != 1 || f.deletes[0] != 1 {
		t.Errorf("deletes %v", f.deletes)
	}
}

func TestKeepsAnItemReopenedDuringItsDeleteAsANewOne(t *testing.T) {
	f := newFake()
	f.library = []LibraryItem{{ID: 1, Name: "Alpha", Text: "A"}}
	f.hold = true
	_, tt := open(t, f, snippets)
	press(t, tt, ui.RoleButton, "Alpha")
	press(t, tt, ui.RoleButton, "Delete")
	press(t, tt, ui.RoleButton, "Delete")
	press(t, tt, ui.RoleButton, "Alpha")
	f.release(t, tt)
	if hasButton(tt, "Delete") || fieldValue(t, tt, "Text") != "A" {
		t.Fatalf("texts %q", tt.Texts())
	}
	f.hold = false
	press(t, tt, ui.RoleButton, "Save Snippet")
	if f.saves[0].ID != 0 {
		t.Errorf("saved %+v", f.saves[0])
	}
}

func TestRereadsTheListAfterAFailedSaveSoADeletedItemCanBeAddedAgain(t *testing.T) {
	f := newFake()
	f.library = []LibraryItem{{ID: 1, Name: "Alpha", Text: "A"}}
	f.saveErr = func(item LibraryItem) error {
		if item.ID != 0 {
			// Another window deleted it meanwhile.
			f.library = nil
			return errors.New("This item was deleted, so it was not saved. Save again to add it as a new item.")
		}
		return nil
	}
	_, tt := open(t, f, snippets)
	press(t, tt, ui.RoleButton, "Alpha")
	press(t, tt, ui.RoleButton, "Save Snippet")
	if !tt.HasText("This item was deleted, so it was not saved. Save again to add it as a new item.") || hasButton(tt, "Delete") {
		t.Fatalf("texts %q", tt.Texts())
	}
	press(t, tt, ui.RoleButton, "Save Snippet")
	if len(f.saves) != 2 || f.saves[1].ID != 0 {
		t.Errorf("saves %+v", f.saves)
	}
}

func TestAsksWithTheSavedNameAndMovesFocusThroughTheInlineDelete(t *testing.T) {
	f := newFake()
	f.library = []LibraryItem{{ID: 3, Name: "Sig", Text: "Hi"}}
	_, tt := open(t, f, snippets)
	press(t, tt, ui.RoleButton, "Sig")
	typeInto(t, tt, "Name", "Other", false)
	press(t, tt, ui.RoleButton, "Delete")
	keep, _ := tt.AccessByRole(ui.RoleButton, "Keep")
	if !tt.HasText("Delete “Sig”?") || !keep.Focused || keep.Description != "Delete “Sig”?" {
		t.Fatalf("Keep %+v; texts %q", keep, tt.Texts())
	}
	press(t, tt, ui.RoleButton, "Keep")
	if !tt.Focused("Delete") {
		t.Error("Delete has no focus after Keep")
	}
	press(t, tt, ui.RoleButton, "Delete")
	press(t, tt, ui.RoleButton, "Delete")
	if len(f.deletes) != 1 || !tt.Focused("Name") || fieldValue(t, tt, "Name") != "" {
		t.Errorf("deletes %v, name focused %v", f.deletes, tt.Focused("Name"))
	}
}

func TestOpensAKeywordItemByItsNameAndKeyword(t *testing.T) {
	f := newFake()
	f.library = []LibraryItem{{ID: 4, Quicklink: true, Name: "Jira", Keyword: "jira", Text: "https://jira.test/{query}"}}
	_, tt := open(t, f, quicklinks)
	press(t, tt, ui.RoleButton, "Jira jira")
	if fieldValue(t, tt, "URL or path") != "https://jira.test/{query}" {
		t.Errorf("texts %q", tt.Texts())
	}
	click(t, tt, "Snippets")
	if !tt.HasText("No snippets yet") {
		t.Errorf("a quicklink shows with the snippets: %q", tt.Texts())
	}
}
