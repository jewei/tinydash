package settings

import (
	"errors"
	"fmt"
	"slices"
	"strings"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/settings"
)

// The tests below port src/settings/Settings.test.tsx, the spec of the
// Solid Settings window, for the settings the Go app keeps.

func TestSavesAChangeAtOnce(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, general)
	flip(t, tt, "Hide when another app is focused")
	f.wantUpdates(t, 1)
	if f.last(t).HideOnBlur || checked(t, tt, "Hide when another app is focused") {
		t.Error("hide on blur is still on")
	}
}

func TestKeepsQuickChangesInOrder(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, general)
	flip(t, tt, "Hide when another app is focused")
	flip(t, tt, "Open at login")
	// The form shows both at once, and the second waits for the first.
	if checked(t, tt, "Hide when another app is focused") || !checked(t, tt, "Open at login") {
		t.Error("the form does not show the changes on their way")
	}
	f.wantUpdates(t, 1)
	f.release(t, tt)
	f.wantUpdates(t, 2)
	f.release(t, tt)
	if s := f.last(t); s.HideOnBlur || !s.LaunchAtLogin {
		t.Errorf("the second change lost the first: %+v", s)
	}
}

func TestShowsWhyAChangeFailedAndRestoresTheSavedValue(t *testing.T) {
	f := newFake()
	f.refuse = func(settings.Settings) error { return errors.New("Could not change open at login: denied.") }
	_, tt := open(t, f, general)
	flip(t, tt, "Open at login")
	if info, ok := tt.AccessByRole(ui.RoleAlert, ""); !ok || !strings.Contains(info.Name, "Could not change open at login") {
		t.Fatalf("no alert: %q", tt.Texts())
	}
	if checked(t, tt, "Open at login") {
		t.Error("the failed value shows")
	}
}

func TestDoesNotSendAFailedChangeAgainWithTheNextOne(t *testing.T) {
	f := newFake()
	f.refuse = func(s settings.Settings) error {
		if s.LaunchAtLogin {
			return errors.New("Could not change open at login.")
		}
		return nil
	}
	_, tt := open(t, f, general)
	flip(t, tt, "Open at login")
	flip(t, tt, "Hide when another app is focused")
	f.wantUpdates(t, 2)
	if s := f.last(t); s.LaunchAtLogin || s.HideOnBlur {
		t.Errorf("sent %+v", s)
	}
	if tt.HasText("Could not change open at login.") {
		t.Error("a later change that worked left the old error")
	}
}

// Clipboard cards stay off until the user turns them on, as in the Rust
// app: the pane then reads the clipboard each time it shows.
func TestTurnsClipboardCardsOn(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, widgets)
	if checked(t, tt, "Clipboard cards") {
		t.Fatal("clipboard cards are on by default")
	}
	flip(t, tt, "Clipboard cards")
	f.wantUpdates(t, 1)
	if !f.last(t).ShowClipboardCards {
		t.Error("the change did not turn clipboard cards on")
	}
}

func TestDescribesEachSettingToScreenReaders(t *testing.T) {
	_, tt := open(t, newFake(), clipboard)
	info, ok := tt.AccessByRole(ui.RoleGroup, "Save clipboard history")
	if !ok || !strings.Contains(info.Description, "unencrypted") {
		t.Errorf("%+v %v", info, ok)
	}
}

func TestCentersALauncherThatWasDraggedAndOnlyThen(t *testing.T) {
	f := newFake()
	f.saved.LauncherPosition = &settings.Position{X: 40, Y: 80}
	_, tt := open(t, f, general)
	if !tt.HasText("Opens where you last dragged it.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	press(t, tt, ui.RoleButton, "Center")
	f.wantUpdates(t, 1)
	if f.last(t).LauncherPosition != nil {
		t.Error("not centered")
	}
	info, _ := tt.AccessByRole(ui.RoleButton, "Center")
	if !info.Disabled || !tt.Focused("Center") {
		t.Errorf("Center %+v, focused %v", info, tt.Focused("Center"))
	}
	press(t, tt, ui.RoleButton, "Center")
	f.wantUpdates(t, 1)
}

func TestChoosesTheAppearance(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, general)
	press(t, tt, ui.RolePopUpButton, "Appearance")
	press(t, tt, ui.RoleOption, "Dark")
	f.wantUpdates(t, 1)
	if f.last(t).Theme != settings.ThemeDark {
		t.Errorf("theme %q", f.last(t).Theme)
	}
}

func TestRecordsANewShortcutWhileTheOldOneIsPaused(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	if !tt.Focused("Launcher shortcut: press the new keys, or Escape to cancel") {
		t.Fatal("the recorder has no focus")
	}
	tt.Key(ui.Alt, ui.KeySpace)
	f.wantUpdates(t, 1)
	if f.last(t).Shortcut != "Alt+Space" || !slices.Equal(f.pauses, []bool{true, false}) {
		t.Errorf("shortcut %q, pauses %v", f.last(t).Shortcut, f.pauses)
	}
	if _, ok := tt.AccessByRole(ui.RoleButton, "Launcher shortcut: Alt Space"); !ok {
		t.Errorf("texts %q", tt.Texts())
	}
}

// Pause, save, and resume run one after another, whenever their replies
// come.
func TestResumesTheShortcutsOnlyAfterTheSave(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	if !tt.Focused("Launcher shortcut: press the new keys, or Escape to cancel") {
		t.Fatal("the recorder has no focus")
	}
	tt.Key(ui.Alt, ui.KeySpace)
	f.wantUpdates(t, 0)
	f.release(t, tt) // paused
	f.wantUpdates(t, 1)
	if !slices.Equal(f.pauses, []bool{true}) {
		t.Fatalf("pauses %v while the shortcut saves", f.pauses)
	}
	f.release(t, tt) // saved
	if !slices.Equal(f.pauses, []bool{true, false}) {
		t.Errorf("pauses %v", f.pauses)
	}
	f.release(t, tt) // resumed
}

func TestRunsEachRecordingRequestAfterTheLast(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	press(t, tt, ui.RoleButton, "Launcher shortcut: press the new keys, or Escape to cancel")
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	if !slices.Equal(f.pauses, []bool{true}) {
		t.Fatalf("pauses %v before the first reply", f.pauses)
	}
	f.release(t, tt)
	f.release(t, tt)
	f.release(t, tt)
	if !slices.Equal(f.pauses, []bool{true, false, true}) || !tt.HasText("Press keys…") {
		t.Errorf("pauses %v, texts %q", f.pauses, tt.Texts())
	}
}

func TestStopsRecordingAndSaysWhyWhenTheShortcutCannotPause(t *testing.T) {
	f := newFake()
	f.pauseErr = func(paused bool) error {
		if paused {
			return errors.New("Could not release the current shortcut.")
		}
		return nil
	}
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	if !tt.HasText("Could not release the current shortcut.") || tt.HasText("Press keys…") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestSaysWhenTheShortcutCannotBeTurnedBackOn(t *testing.T) {
	f := newFake()
	f.pauseErr = func(paused bool) error {
		if !paused {
			return errors.New("Could not register Alt+Space.")
		}
		return nil
	}
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	tt.Key(ui.Alt, ui.KeySpace)
	if !tt.HasText("Could not register Alt+Space.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestCancelsRecordingOnASecondClickOrEscape(t *testing.T) {
	f := newFake()
	m, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	press(t, tt, ui.RoleButton, "Launcher shortcut: press the new keys, or Escape to cancel")
	if tt.HasText("Press keys…") {
		t.Fatal("a second click kept recording")
	}
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	tt.Key(0, ui.KeyEscape)
	if m.rec.On() || !slices.Equal(f.pauses, []bool{true, false, true, false}) {
		t.Errorf("recording %v, pauses %v", m.rec.On(), f.pauses)
	}
	f.wantUpdates(t, 0)
}

func TestTurnsTheShortcutsBackOnWhenTheWindowClosesWhileRecording(t *testing.T) {
	f := newFake()
	m, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	m.Close()
	if !slices.Equal(f.pauses, []bool{true, false}) {
		t.Errorf("pauses %v", f.pauses)
	}
}

func TestStopsAddingFoldersWhenTheListIsFull(t *testing.T) {
	f := newFake()
	f.saved.FileSearchFolders = nil
	for i := range folderLimit {
		f.saved.FileSearchFolders = append(f.saved.FileSearchFolders, fmt.Sprintf("~/Folder%d", i))
	}
	_, tt := open(t, f, search)
	if !tt.HasText("The list is full (50). Remove one to add another.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	if folder, _ := tt.AccessByRole(ui.RoleButton, "Add to Folder to add"); !folder.Disabled {
		t.Error("Add is on with a full list")
	}
	if skip, _ := tt.AccessByRole(ui.RoleButton, "Add to Folder name to skip"); skip.Disabled {
		t.Error("the other list's Add is off")
	}
	press(t, tt, ui.RoleButton, "Remove ~/Folder0")
	if !tt.Focused("Folder to add") {
		t.Error("Remove left the focus nowhere")
	}
}

func TestSaysWhenAFolderIsAlreadyInTheList(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, search)
	typeInto(t, tt, "Folder to add", "~/Desktop", true)
	if !tt.HasText("“~/Desktop” is already in the list.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	f.wantUpdates(t, 0)
	press(t, tt, ui.RoleButton, "Remove ~/Desktop")
	if tt.HasText("“~/Desktop” is already in the list.") {
		t.Error("the notice stays after another change")
	}
}

func TestAddsAClockCityAndShowsWhyAnUnknownOneIsRefused(t *testing.T) {
	f := newFake()
	f.refuse = func(s settings.Settings) error {
		if slices.Contains(s.ClockCities, "Atlantis") {
			return errors.New("TinyDash does not know “Atlantis”.")
		}
		return nil
	}
	m, tt := open(t, f, widgets)
	typeInto(t, tt, "City to add", "Tokyo", true)
	if _, ok := tt.AccessByRole(ui.RoleButton, "Remove Tokyo"); !ok || !slices.Equal(f.last(t).ClockCities, []string{"Tokyo"}) {
		t.Fatalf("cities %q; texts %q", f.last(t).ClockCities, tt.Texts())
	}
	if m.cities.draft != "" {
		t.Errorf("the field kept %q after a save", m.cities.draft)
	}
	typeInto(t, tt, "City to add", "Atlantis", true)
	if info, ok := tt.AccessByRole(ui.RoleStatus, ""); !ok || info.Name != "TinyDash does not know “Atlantis”." {
		t.Errorf("status %+v", info)
	}
	if m.cities.draft != "Atlantis" {
		t.Errorf("the field holds %q", m.cities.draft)
	}
}

func TestSaysWhenAClockCityIsAlreadyInTheListInAnotherCase(t *testing.T) {
	f := newFake()
	f.saved.ClockCities = []string{"Tokyo"}
	_, tt := open(t, f, widgets)
	typeInto(t, tt, "City to add", "tokyo", true)
	if !tt.HasText("“tokyo” is already in the list.") {
		t.Errorf("texts %q", tt.Texts())
	}
	f.wantUpdates(t, 0)
}

func TestKeepsARefusedFolderInTheFieldAndSaysWhyNextToIt(t *testing.T) {
	f := newFake()
	f.refuse = func(settings.Settings) error { return errors.New("“Projects” is not a full folder path.") }
	m, tt := open(t, f, search)
	typeInto(t, tt, "Folder to add", "Projects", true)
	if m.folders.notice != "“Projects” is not a full folder path." || m.folders.draft != "Projects" {
		t.Errorf("notice %q, draft %q", m.folders.notice, m.folders.draft)
	}
}

func TestAppliesEachFolderChangeToTheSavedList(t *testing.T) {
	f := newFake()
	f.hold = true
	f.refuse = func(s settings.Settings) error {
		if slices.Contains(s.FileSearchFolders, "Projects") {
			return errors.New("“Projects” is not a full folder path.")
		}
		return nil
	}
	m, tt := open(t, f, search)
	typeInto(t, tt, "Folder to add", "Projects", true)
	press(t, tt, ui.RoleButton, "Remove ~/Desktop")
	f.release(t, tt)
	if m.folders.notice != "“Projects” is not a full folder path." {
		t.Errorf("notice %q", m.folders.notice)
	}
	f.wantUpdates(t, 2)
	f.release(t, tt)
	if got := f.saved.FileSearchFolders; !slices.Equal(got, []string{"~/Documents", "~/Downloads"}) {
		t.Errorf("saved %q", got)
	}
}

func TestShowsASaveErrorEvenWhenTheListIsFull(t *testing.T) {
	f := newFake()
	f.saved.FileSearchFolders = nil
	for i := range folderLimit {
		f.saved.FileSearchFolders = append(f.saved.FileSearchFolders, fmt.Sprintf("~/Folder%d", i))
	}
	f.refuse = func(settings.Settings) error { return errors.New("“Projects” is not a full folder path.") }
	_, tt := open(t, f, search)
	press(t, tt, ui.RoleButton, "Remove ~/Folder3")
	if !tt.HasText("“Projects” is not a full folder path.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestSavesAFolderAddedDuringAFailingSaveOnItsOwn(t *testing.T) {
	f := newFake()
	f.hold = true
	f.refuse = func(s settings.Settings) error {
		if slices.Contains(s.FileSearchFolders, "Projects") {
			return errors.New("“Projects” is not a full folder path.")
		}
		return nil
	}
	_, tt := open(t, f, search)
	typeInto(t, tt, "Folder to add", "Projects", true)
	// Leaving the section and coming back must not change what is sent.
	click(t, tt, "General")
	click(t, tt, "Search")
	typeInto(t, tt, "Folder to add", "~/Work", true)
	f.release(t, tt)
	f.release(t, tt)
	want := append(settings.Default().FileSearchFolders, "~/Work")
	if got := f.saved.FileSearchFolders; !slices.Equal(got, want) {
		t.Errorf("saved %q, want %q", got, want)
	}
}

func TestNeverSendsAnEmojiLanguageAgainAfterItsSaveFailed(t *testing.T) {
	f := newFake()
	f.hold = true
	f.refuse = func(s settings.Settings) error {
		if slices.Contains(s.EmojiLanguages, "zh") {
			return errRefused
		}
		return nil
	}
	_, tt := open(t, f, search)
	flip(t, tt, "Chinese (Simplified)")
	flip(t, tt, "Malay")
	f.release(t, tt)
	f.release(t, tt)
	if got := f.saved.EmojiLanguages; !slices.Equal(got, []settings.EmojiLanguage{"ms"}) {
		t.Errorf("saved %q", got)
	}
}

func TestRestoresAnEmptiedNumberInsteadOfSavingZero(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, clipboard)
	typeInto(t, tt, "Entries to keep", "", true)
	f.wantUpdates(t, 0)
	if info, _ := tt.AccessByRole(ui.RoleTextField, "Entries to keep"); info.Value != "200" {
		t.Errorf("field %q", info.Value)
	}
	typeInto(t, tt, "Entries to keep", "5000", false)
	f.wantUpdates(t, 0)
	// Leaving the field saves it, within its limit.
	flip(t, tt, "Save clipboard history")
	if f.updates[0].ClipboardHistoryLimit != settings.ClipboardLimitMax {
		t.Errorf("limit %d", f.updates[0].ClipboardHistoryLimit)
	}
}

func TestOffersImageAndFileHistoryOnlyWhereTheOSCanSaveThem(t *testing.T) {
	f := newFake()
	m, tt := open(t, f, clipboard)
	if _, ok := tt.AccessByRole(ui.RoleSwitch, "Save images"); !ok {
		t.Fatal("no Save images")
	}
	m.richClipboard = false
	tt.Frame()
	for _, name := range []string{"Save images", "Save copied files"} {
		if _, ok := tt.AccessByRole(ui.RoleSwitch, name); ok {
			t.Errorf("%s shows where the OS saves text only", name)
		}
	}
}

func TestOffersSpotlightOnlyOnMacOS(t *testing.T) {
	m, tt := open(t, newFake(), search)
	if _, ok := tt.AccessByRole(ui.RoleSwitch, "Also search with Spotlight"); ok {
		t.Error("Spotlight off macOS")
	}
	m.mac = true
	tt.Frame()
	if _, ok := tt.AccessByRole(ui.RoleSwitch, "Also search with Spotlight"); !ok {
		t.Error("no Spotlight on macOS")
	}
}

func TestReturnsFocusToClearHistoryAfterItsDialogCloses(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, clipboard)
	press(t, tt, ui.RoleButton, "Clear History…")
	if !tt.Focused("Cancel") {
		t.Error("Cancel has no focus")
	}
	press(t, tt, ui.RoleButton, "Cancel")
	if !tt.Focused("Clear History…") || f.clears != 0 {
		t.Errorf("focused %v, clears %d", tt.Focused("Clear History…"), f.clears)
	}
	press(t, tt, ui.RoleButton, "Clear History…")
	press(t, tt, ui.RoleButton, "Clear History")
	if f.clears != 1 {
		t.Errorf("clears %d", f.clears)
	}
}

func TestListsHiddenResultsAndShowsOneAgain(t *testing.T) {
	f := newFake()
	f.hidden = []engine.Listed{{ID: "app:Helper", Title: "Helper", Subtitle: "Utility"}}
	_, tt := open(t, f, search)
	press(t, tt, ui.RoleButton, "Show Helper again")
	if !slices.Equal(f.unhides, []core.ID{"app:Helper"}) {
		t.Errorf("unhid %v", f.unhides)
	}
	if !tt.HasText("None. To hide one, choose Hide from Results in its actions (Ctrl K).") || !tt.Focused("Hidden results") {
		t.Errorf("texts %q, list focused %v", tt.Texts(), tt.Focused("Hidden results"))
	}
}

func TestMovesFocusToTheNextHiddenResultAfterShowAgain(t *testing.T) {
	f := newFake()
	for _, title := range []string{"One", "Two", "Three"} {
		f.hidden = append(f.hidden, engine.Listed{ID: core.ID("system:" + title), Title: title, Subtitle: "Command"})
	}
	_, tt := open(t, f, search)
	press(t, tt, ui.RoleButton, "Show Two again")
	if !tt.Focused("Show Three again") {
		t.Error("the next row has no focus")
	}
	press(t, tt, ui.RoleButton, "Show Three again")
	if !tt.Focused("Show One again") {
		t.Error("the row before has no focus at the end")
	}
}

// order is the tabs the last change sent, with hidden ones marked.
func order(s settings.Settings) []string {
	var out []string
	for _, tab := range s.Tabs {
		name := string(tab.Category)
		if !tab.Shown {
			name += " (hidden)"
		}
		out = append(out, name)
	}
	return out
}

func TestMovesAndHidesLauncherTabsKeepingFocusOnThem(t *testing.T) {
	f := newFake()
	calls := 0
	f.refuse = func(settings.Settings) error {
		calls++
		if calls == 3 {
			return errRefused
		}
		return nil
	}
	_, tt := open(t, f, general)
	if up, _ := tt.AccessByRole(ui.RoleButton, "Move Apps up"); !up.Disabled {
		t.Error("the first tab moves up")
	}
	press(t, tt, ui.RoleButton, "Move Files up")
	if got := order(f.last(t)); !slices.Equal(got, []string{"files", "apps", "clipboard", "snippets", "emoji", "system"}) {
		t.Errorf("order %q", got)
	}
	// Files is first now, so its Up is off and the focus moves to Down.
	if !tt.Focused("Move Files down") {
		t.Error("the focus did not follow the moved tab")
	}
	flip(t, tt, "Show Emoji")
	if got := order(f.last(t)); !slices.Equal(got, []string{"files", "apps", "clipboard", "snippets", "emoji (hidden)", "system"}) {
		t.Errorf("order %q", got)
	}
	// A failed move puts the row back, and the focus stays on its button.
	press(t, tt, ui.RoleButton, "Move Files down")
	if !tt.HasText(errRefused.Error()) || !tt.Focused("Move Files down") {
		t.Errorf("texts %q, focused %v", tt.Texts(), tt.Focused("Move Files down"))
	}
	if got := order(f.saved); !slices.Equal(got, []string{"files", "apps", "clipboard", "snippets", "emoji (hidden)", "system"}) {
		t.Errorf("saved order %q", got)
	}
}

func TestLeavesFocusPutElsewhereWhileATabMoves(t *testing.T) {
	f := newFake()
	f.hold = true
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Move Files up")
	if !tt.Focused("Move Files down") {
		t.Fatal("the focus did not follow the moving tab")
	}
	press(t, tt, ui.RoleSwitch, "Show Emoji")
	f.release(t, tt)
	if !tt.Focused("Show Emoji") {
		t.Error("the save took the focus back")
	}
}

func TestKeepsATabsResultsOutOfAllButNeverHidesItToo(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, general)
	emoji := func() settings.Tab {
		i := slices.IndexFunc(f.last(t).Tabs, func(tab settings.Tab) bool { return tab.Category == core.CategoryEmoji })
		return f.last(t).Tabs[i]
	}
	flip(t, tt, "Emoji in All")
	if got := emoji(); got != (settings.Tab{Category: core.CategoryEmoji, Shown: true, InAll: false}) {
		t.Errorf("%+v", got)
	}
	// Hiding it too would leave its results nowhere, so In All turns back
	// on, and the focus stays on the switch that was clicked.
	flip(t, tt, "Show Emoji")
	if got := emoji(); got != (settings.Tab{Category: core.CategoryEmoji, Shown: false, InAll: true}) {
		t.Errorf("%+v", got)
	}
	if !checked(t, tt, "Emoji in All") || !tt.Focused("Show Emoji") {
		t.Errorf("in All %v, focused %v", checked(t, tt, "Emoji in All"), tt.Focused("Show Emoji"))
	}
	// Taking a hidden tab out of All shows it again.
	flip(t, tt, "Emoji in All")
	if got := emoji(); got != (settings.Tab{Category: core.CategoryEmoji, Shown: true, InAll: false}) {
		t.Errorf("%+v", got)
	}
}

func TestReadsChangesMadeElsewhere(t *testing.T) {
	f := newFake()
	m, tt := open(t, f, search)
	f.saved.CurrencyRatesEnabled = true
	f.hidden = []engine.Listed{{ID: "system:lock", Title: "Lock Screen", Subtitle: "System"}}
	m.Stale()
	tt.Frame()
	if !checked(t, tt, "Currency rates") || !tt.HasText("Lock Screen") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// Ports AliasList.tsx and HotkeyList.tsx: Settings > Search lists them,
// and Remove removes one; the focus goes to the row in its place.
func TestListsAliasesAndRemovesOne(t *testing.T) {
	f := newFake()
	f.aliases = []engine.Listed{
		{ID: "app:Code", Title: "Visual Studio Code", Subtitle: "Applications", Value: "vsc"},
		{ID: "system:sleep", Title: "Sleep", Subtitle: "Command", Value: "zz"},
	}
	_, tt := open(t, f, search)
	if !tt.HasText("vsc") || !tt.HasText("Visual Studio Code") {
		t.Fatalf("texts %q", tt.Texts())
	}
	press(t, tt, ui.RoleButton, "Remove the alias vsc of Visual Studio Code")
	if !slices.Equal(f.aliasRemovals, []core.ID{"app:Code"}) || tt.HasText("Visual Studio Code") {
		t.Errorf("removed %v; texts %q", f.aliasRemovals, tt.Texts())
	}
	if !tt.Focused("Remove the alias zz of Sleep") {
		t.Error("the next row has no focus")
	}
	press(t, tt, ui.RoleButton, "Remove the alias zz of Sleep")
	if !tt.HasText("None. To add one, choose Add Alias in a result's actions (Ctrl K).") || !tt.Focused("Aliases") {
		t.Errorf("texts %q, list focused %v", tt.Texts(), tt.Focused("Aliases"))
	}
}

func TestListsHotkeysAndSaysWhyOneStays(t *testing.T) {
	f := newFake()
	f.hotkeys = []engine.Listed{{ID: "app:Safari", Title: "Safari", Subtitle: "Applications", Value: "Control+Alt+S"}}
	f.removeErr = errRefused
	_, tt := open(t, f, search)
	for _, text := range []string{"Ctrl", "Alt", "S", "Safari"} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
	press(t, tt, ui.RoleButton, "Remove the hotkey of Safari")
	if !slices.Equal(f.hotkeyRemoval, []core.ID{"app:Safari"}) || !tt.HasText(errRefused.Error()) || !tt.HasText("Safari") {
		t.Errorf("removed %v; texts %q", f.hotkeyRemoval, tt.Texts())
	}
	f.removeErr = nil
	press(t, tt, ui.RoleButton, "Remove the hotkey of Safari")
	if !tt.HasText("None. To add one, choose Set Hotkey in a result's actions (Ctrl K).") || tt.HasText(errRefused.Error()) {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestShowsTheTabsUnderGeneral(t *testing.T) {
	f := newFake()
	_, tt := open(t, f, general)
	if _, ok := tt.AccessByRole(ui.RoleList, "Tabs, in order"); !ok {
		t.Error("General has no tabs")
	}
	click(t, tt, string(search))
	if _, ok := tt.AccessByRole(ui.RoleList, "Tabs, in order"); ok {
		t.Error("Search still has the tabs")
	}
}

// A refused new shortcut keeps its reason: the resume error after it is
// about the old shortcut.
func TestAResumeErrorDoesNotReplaceTheShortcutSaveError(t *testing.T) {
	f := newFake()
	f.refuse = func(settings.Settings) error { return errors.New("Could not register Alt+Space: taken.") }
	f.pauseErr = func(paused bool) error {
		if !paused {
			return errors.New("Could not register Control+Shift+Space: taken.")
		}
		return nil
	}
	_, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	tt.Key(ui.Alt, ui.KeySpace)
	if !tt.HasText("Could not register Alt+Space: taken.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// A refused shortcut shows its reason once, under the recorder; a refused
// list change shows in the page banner, as an alert, and in the list.
func TestEachRefusalShowsInItsPlace(t *testing.T) {
	f := newFake()
	f.refuse = func(settings.Settings) error { return errors.New("Could not save settings.json.") }
	m, tt := open(t, f, general)
	press(t, tt, ui.RoleButton, "Launcher shortcut: Ctrl Shift Space")
	tt.Key(ui.Alt, ui.KeySpace)
	tt.Frame()
	if m.err != "" || m.rec.Err() != "Could not save settings.json." {
		t.Errorf("banner %q, recorder %q", m.err, m.rec.Err())
	}
	f.refuse = func(settings.Settings) error { return errors.New("Could not save the folder list.") }
	m.folders.draft = "~/Notes"
	m.addTo(&m.folders, nil, folderLimit, false, func(s *settings.Settings) *[]string { return &s.FileSearchFolders })
	tt.Frame()
	if m.err != "Could not save the folder list." {
		t.Errorf("banner %q after a refused list change", m.err)
	}
	if _, ok := tt.AccessByRole(ui.RoleAlert, "Could not save the folder list."); !ok {
		t.Errorf("no alert; texts %q", tt.Texts())
	}
}
