package launcher

import (
	"errors"
	"slices"
	"testing"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// The tests below port src/launcher/Launcher.test.tsx, the spec of the
// Solid launcher, for the features the Go app keeps.

func TestSearchesAsTheUserTypes(t *testing.T) {
	f := newFake(func(q string, _ core.Category) []core.Result {
		if q == "saf" {
			return []core.Result{safari}
		}
		return nil
	})
	_, tt := open(t, f)
	tt.Type("saf")
	if !tt.HasText("Safari") {
		t.Fatalf("texts %q", tt.Texts())
	}
	if got := f.lastSearch(); got != (searched{"saf", core.CategoryAll}) {
		t.Errorf("last search %+v", got)
	}
}

func TestRunsTheMainActionWithItsResultForRanking(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{safari.Actions[0].Action, safari.ID})
}

func TestRunsTheSecondActionWithModEnterWithoutCountingUse(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyEnter)
	wantRuns(t, f, ran{safari.Actions[1].Action, ""})
}

func TestNeverRepeatsAnActionWhileAKeyIsHeld(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tt.KeyRepeat(0, ui.KeyEnter)
	tt.KeyRepeat(ui.Cmd, ui.KeyEnter)
	tt.KeyRepeat(ui.Cmd, ui.Key1)
	wantRuns(t, f)
}

func TestTypesAKeywordFromTheQuestionMarkList(t *testing.T) {
	keyword := core.Result{ID: "keyword:Google", Kind: core.KindKeyword, Title: "g, google",
		Actions: []core.ResultAction{core.Do("Use Keyword", core.TypeQuery{Text: "g "})}}
	f := newFake(func(q string, _ core.Category) []core.Result {
		if q == "?" {
			return []core.Result{keyword}
		}
		return nil
	})
	m, tt := open(t, f)
	tt.Type("?")
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{core.TypeQuery{Text: "g "}, keyword.ID})
	// The engine checks the text and sends it back to type.
	m.TypeQuery("g ")
	tt.Frame()
	if m.query != "g " || f.lastSearch() != (searched{"g ", core.CategoryAll}) || !tt.Focused("Search") {
		t.Errorf("query %q, last search %+v, search focused %v", m.query, f.lastSearch(), tt.Focused("Search"))
	}
	// Typing a keyword is not a search to bring back.
	m.Show("")
	tt.Frame()
	tt.Key(0, ui.KeyUp)
	if m.query != "" {
		t.Errorf("Up brought back %q", m.query)
	}
}

func TestUpBringsBackPastSearchesFromAnEmptyField(t *testing.T) {
	f := newFake(always(safari))
	m, tt := open(t, f)
	for _, q := range []string{"saf", "calc", "saf"} {
		m.Show("")
		tt.Frame()
		tt.Type(q)
		tt.Key(0, ui.KeyEnter)
	}
	m.Show("")
	tt.Frame()
	// Newest first, each query once, stopping at the oldest.
	for _, want := range []string{"saf", "calc", "calc"} {
		tt.Key(0, ui.KeyUp)
		if m.query != want {
			t.Fatalf("Up gave %q, want %q", m.query, want)
		}
	}
	if f.lastSearch() != (searched{"calc", core.CategoryAll}) {
		t.Errorf("last search %+v", f.lastSearch())
	}
	// An edited query is the user's own: Up moves in the results again.
	tt.Type("u")
	tt.Key(0, ui.KeyUp)
	if m.query != "calcu" {
		t.Errorf("query %q after Up on an edited search", m.query)
	}
}

func TestUpBelowTheFirstRowMovesUpTheResults(t *testing.T) {
	f := newFake(always(safari, app("Notes")))
	m, tt := open(t, f)
	tt.Type("saf")
	tt.Key(0, ui.KeyEnter)
	m.Show("")
	tt.Frame()
	tt.Key(0, ui.KeyDown)
	tt.Key(0, ui.KeyUp)
	if m.query != "" || selectedRow(tt, "Safari", "Notes") != "Safari" {
		t.Errorf("query %q, selected %q", m.query, selectedRow(tt, "Safari", "Notes"))
	}
}

func TestAHeldUpStopsAtTheFirstRow(t *testing.T) {
	f := newFake(always(safari, app("Notes")))
	m, tt := open(t, f)
	tt.Type("saf")
	tt.Key(0, ui.KeyEnter)
	m.Show("")
	tt.Frame()
	tt.Key(0, ui.KeyDown)
	tt.KeyRepeat(0, ui.KeyUp)
	tt.KeyRepeat(0, ui.KeyUp)
	if m.query != "" || selectedRow(tt, "Safari", "Notes") != "Safari" {
		t.Errorf("query %q, selected %q", m.query, selectedRow(tt, "Safari", "Notes"))
	}
	tt.Key(0, ui.KeyUp)
	if m.query != "saf" {
		t.Errorf("a new Up gave %q", m.query)
	}
}

func TestAsksBeforeADestructiveActionWithCancelFocused(t *testing.T) {
	f := newFake(always(restart))
	_, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	if _, ok := tt.AccessByRole(ui.RoleAlertDialog, "Confirm"); !ok || !tt.HasText("Restart the computer?") {
		t.Fatalf("no confirmation: %q", tt.Texts())
	}
	if !tt.Focused("Cancel") {
		t.Error("Cancel has no focus")
	}
	// A held Enter must not answer the dialog it just opened.
	tt.KeyRepeat(0, ui.KeyEnter)
	if _, ok := tt.AccessByRole(ui.RoleAlertDialog, "Confirm"); !ok {
		t.Fatal("a held Enter answered the confirmation")
	}
	click(t, tt, "Cancel")
	wantRuns(t, f)
	tt.Key(0, ui.KeyEnter)
	pressButton(t, tt, "Run")
	wantRuns(t, f, ran{restart.Actions[0].Action, restart.ID})
	if !tt.Focused("Search") {
		t.Error("the search field has no focus after the confirmation")
	}
}

func TestCancelsAConfirmationWithEscapeOrAPressOutside(t *testing.T) {
	f := newFake(always(restart))
	_, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	tt.ClickAt(5, 5)
	if _, ok := tt.AccessByRole(ui.RoleAlertDialog, "Confirm"); ok {
		t.Fatal("a press on the backdrop kept the confirmation")
	}
	tt.Key(0, ui.KeyEnter)
	tt.Key(0, ui.KeyEscape)
	if _, ok := tt.AccessByRole(ui.RoleAlertDialog, "Confirm"); ok {
		t.Fatal("Escape kept the confirmation")
	}
	wantRuns(t, f)
	if f.hidden != 0 {
		t.Error("Escape in the confirmation hid the launcher")
	}
}

func TestAsksForAnAliasAndSendsTheOneTyped(t *testing.T) {
	named := withAction(safari, core.Do("Change Alias “saf”…", core.SetAlias{ID: safari.ID, Alias: "saf"}))
	f := newFake(always(named))
	m, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	chooseItem(t, tt, "Change Alias “saf”…")
	if _, ok := tt.AccessByRole(ui.RoleDialog, "Alias for Safari"); !ok {
		t.Fatalf("no alias dialog: %q", tt.Texts())
	}
	if m.alias.text != "saf" || !tt.Focused("Alias for Safari") {
		t.Fatalf("field %q, focused %v", m.alias.text, tt.Focused("Alias for Safari"))
	}
	tt.Command("selectAll")
	tt.Type("sf")
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{core.SetAlias{ID: safari.ID, Alias: "sf"}, ""})
	if m.alias != nil || !tt.Focused("Search") {
		t.Errorf("dialog open %v, search focused %v", m.alias != nil, tt.Focused("Search"))
	}
}

func TestKeepsTheAliasDialogOpenWithTheReasonWhenSavingFails(t *testing.T) {
	named := core.Result{ID: safari.ID, Kind: core.KindApp, Title: "Safari",
		Actions: []core.ResultAction{core.Do("Add Alias…", core.SetAlias{ID: safari.ID})}}
	f := newFake(always(named))
	reason := "“sf” is already the alias of Shortcuts. Choose another word."
	f.fail = func(core.Action) error { return errors.New(reason) }
	m, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	tt.Type("sf")
	click(t, tt, "Save Alias")
	if info, ok := tt.AccessByRole(ui.RoleAlert, ""); !ok || info.Name != reason {
		t.Fatalf("alert %+v %v", info, ok)
	}
	if m.alias == nil || m.alias.text != "sf" {
		t.Fatal("the dialog closed or lost the alias")
	}
	f.fail = nil
	tt.Type("r")
	tt.Key(0, ui.KeyEnter)
	if m.alias != nil {
		t.Fatal("the dialog stayed open after the save")
	}
	wantRuns(t, f, ran{core.SetAlias{ID: safari.ID, Alias: "sf"}, ""}, ran{core.SetAlias{ID: safari.ID, Alias: "sfr"}, ""})
}

func TestClosesTheAliasDialogWhenTheLauncherOpensAgainEvenDuringASave(t *testing.T) {
	named := core.Result{ID: safari.ID, Kind: core.KindApp, Title: "Safari",
		Actions: []core.ResultAction{core.Do("Add Alias…", core.SetAlias{ID: safari.ID})}}
	f := newFake(always(named))
	f.hold = true
	m, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	// A held Enter does not save.
	tt.KeyRepeat(0, ui.KeyEnter)
	wantRuns(t, f)
	tt.Type("sf")
	tt.Key(0, ui.KeyEnter)
	// While it saves, Escape and Cancel do not pretend to cancel it.
	tt.Key(0, ui.KeyEscape)
	_ = tt.Click("Cancel")
	if m.alias == nil {
		t.Fatal("the dialog closed during its save")
	}
	m.Show("")
	tt.Frame()
	if m.alias != nil {
		t.Fatal("the dialog stayed open as the launcher opened again")
	}
	// A dialog opened meanwhile is free, and says the old save still runs.
	tt.Key(0, ui.KeyEnter)
	click(t, tt, "Save Alias")
	if !tt.HasText("Another action is still running. Save again when it ends.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	click(t, tt, "Cancel")
	// The late reply neither reopens the dialog nor shows its error.
	tt.Key(ui.Cmd, ui.KeyK)
	f.finish(errors.New("Could not save."))
	tt.Frame()
	if m.alias != nil || tt.HasText("Could not save.") || m.menu == nil {
		t.Errorf("alias %v, texts %q, menu %v", m.alias, tt.Texts(), m.menu != nil)
	}
}

func TestLeavesTheAliasDialogFreeWhileAnotherActionRuns(t *testing.T) {
	named := core.Result{ID: safari.ID, Kind: core.KindApp, Title: "Safari", Actions: []core.ResultAction{
		safari.Actions[0], core.Do("Add Alias…", core.SetAlias{ID: safari.ID}),
	}}
	f := newFake(always(named))
	f.hold = true
	m, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	tt.Key(ui.Cmd, ui.KeyK)
	chooseItem(t, tt, "Add Alias…")
	click(t, tt, "Save Alias")
	if !tt.HasText("Another action is still running. Save again when it ends.") || len(f.runs) != 1 {
		t.Fatalf("texts %q, runs %+v", tt.Texts(), f.runs)
	}
	click(t, tt, "Cancel")
	if m.alias != nil {
		t.Error("Cancel kept the dialog")
	}
}

func TestClosesTheAliasDialogWithEscapeAndSavesNothing(t *testing.T) {
	named := core.Result{ID: safari.ID, Kind: core.KindApp, Title: "Safari",
		Actions: []core.ResultAction{core.Do("Add Alias…", core.SetAlias{ID: safari.ID})}}
	f := newFake(always(named))
	m, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	tt.Type("x")
	tt.Key(0, ui.KeyEscape)
	if m.alias != nil || len(f.runs) != 0 || f.hidden != 0 {
		t.Errorf("alias %v, runs %+v, hidden %d", m.alias, f.runs, f.hidden)
	}
}

func hotkeyResult(shortcut string) core.Result {
	label := "Set Hotkey…"
	if shortcut != "" {
		label = "Change Hotkey…"
	}
	return withAction(safari, core.Do(label, core.SetHotkey{ID: safari.ID, Shortcut: shortcut}))
}

func openHotkey(t *testing.T, tt *ui.Tester, label string) {
	t.Helper()
	tt.Key(ui.Cmd, ui.KeyK)
	chooseItem(t, tt, label)
	if _, ok := tt.AccessByRole(ui.RoleDialog, "Hotkey for Safari"); !ok {
		t.Fatalf("no hotkey dialog: %q", tt.Texts())
	}
}

func TestRecordsAHotkeyWithTheShortcutsPausedAndSavesItAtOnce(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	m, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	if !tt.Focused("Hotkey for Safari: press the new keys, or Escape to cancel") {
		t.Error("the recorder has no focus")
	}
	if !slices.Equal(f.paused, []bool{true}) {
		t.Fatalf("paused %v", f.paused)
	}
	// Keys without a modifier are not a hotkey, and never reach the launcher.
	tt.Key(0, ui.KeyEnter)
	tt.Key(0, ui.KeyTab)
	wantRuns(t, f)
	tt.Key(ui.Ctrl|ui.Alt, ui.KeyK)
	wantRuns(t, f, ran{core.SetHotkey{ID: safari.ID, Shortcut: "Control+Alt+K"}, ""})
	if m.hotkey != nil || !slices.Equal(f.paused, []bool{true, false}) || !tt.Focused("Search") {
		t.Errorf("dialog %v, paused %v, search focused %v", m.hotkey, f.paused, tt.Focused("Search"))
	}
}

func TestKeepsTheHotkeyDialogOpenWithTheReasonAndRemovesAHotkey(t *testing.T) {
	f := newFake(always(hotkeyResult("Control+Alt+K")))
	reason := "These keys already run Notes. Choose other keys."
	f.fail = func(a core.Action) error {
		if a.(core.SetHotkey).Shortcut != "" {
			return errors.New(reason)
		}
		return nil
	}
	m, tt := open(t, f)
	openHotkey(t, tt, "Change Hotkey…")
	tt.Key(ui.Ctrl|ui.Alt, ui.KeyN)
	if info, ok := tt.AccessByRole(ui.RoleAlert, ""); !ok || info.Name != reason {
		t.Fatalf("alert %+v %v", info, ok)
	}
	// The failed keys were not saved, so the shortcuts run the old ones again.
	if !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("paused %v", f.paused)
	}
	click(t, tt, "Remove Hotkey")
	wantRuns(t, f,
		ran{core.SetHotkey{ID: safari.ID, Shortcut: "Control+Alt+N"}, ""},
		ran{core.SetHotkey{ID: safari.ID, Shortcut: ""}, ""})
	if m.hotkey != nil {
		t.Error("the dialog stayed open after Remove Hotkey")
	}
}

func TestClosesTheHotkeyDialogWithEscapeOnceItStopsRecording(t *testing.T) {
	f := newFake(always(hotkeyResult("")))
	m, tt := open(t, f)
	openHotkey(t, tt, "Set Hotkey…")
	tt.Key(0, ui.KeyEscape)
	if _, ok := tt.Access("Hotkey for Safari: None"); !ok || m.hotkey == nil {
		t.Fatalf("Escape did not only stop recording: %q", tt.Texts())
	}
	tt.Key(0, ui.KeyEscape)
	if m.hotkey != nil || f.hidden != 0 || len(f.runs) != 0 {
		t.Errorf("dialog %v, hidden %d, runs %+v", m.hotkey, f.hidden, f.runs)
	}
	if !slices.Equal(f.paused, []bool{true, false}) {
		t.Errorf("paused %v", f.paused)
	}
}

func TestActsOnceOnAKeyHeldInTheHotkeyDialog(t *testing.T) {
	f := newFake(always(hotkeyResult("Control+Alt+K")))
	m, tt := open(t, f)
	openHotkey(t, tt, "Change Hotkey…")
	// Escape stops recording; its repeats do not close the dialog too.
	tt.Key(0, ui.KeyEscape)
	tt.KeyRepeat(0, ui.KeyEscape)
	if m.hotkey == nil {
		t.Fatal("a held Escape closed the dialog")
	}
	// A held Enter neither starts recording again nor clicks a button.
	tt.KeyRepeat(0, ui.KeyEnter)
	if m.hotkey.rec.On() {
		t.Fatal("a held Enter started recording")
	}
	tt.Key(0, ui.KeyTab)
	if !tt.Focused("Remove Hotkey") {
		t.Fatalf("Tab did not go to Remove Hotkey")
	}
	tt.KeyRepeat(0, ui.KeyEnter)
	wantRuns(t, f)
	if f.hidden != 0 {
		t.Error("the launcher hid")
	}
	tt.Key(0, ui.KeyEscape)
	if m.hotkey != nil {
		t.Error("a new Escape kept the dialog")
	}
}

func TestAHotkeySaveThatEndsLateLeavesANewerDialogOpen(t *testing.T) {
	f := newFake(always(hotkeyResult("Control+Alt+K")))
	f.hold = true
	m, tt := open(t, f)
	openHotkey(t, tt, "Change Hotkey…")
	click(t, tt, "Remove Hotkey")
	if info, ok := tt.Access("Remove Hotkey"); !ok || !info.Disabled {
		t.Errorf("Remove Hotkey during its save: %+v", info)
	}
	f.finish(errors.New("Could not remove the hotkey."))
	tt.Frame()
	if !tt.HasText("Could not remove the hotkey.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	click(t, tt, "Remove Hotkey")
	m.Show("")
	tt.Frame()
	openHotkey(t, tt, "Change Hotkey…")
	f.finish(nil)
	tt.Frame()
	if m.hotkey == nil {
		t.Fatal("an old save closed the dialog opened since")
	}
	// The newer dialog records, so the shortcuts stay paused.
	if f.paused[len(f.paused)-1] != true {
		t.Errorf("paused %v", f.paused)
	}
}

func TestMovesThroughCategoriesWithTab(t *testing.T) {
	f := newFake(always())
	_, tt := open(t, f)
	if _, ok := tt.AccessByRole(ui.RoleTabList, "Categories"); !ok {
		t.Fatal("no tab list")
	}
	tt.Key(0, ui.KeyTab)
	if f.lastSearch() != (searched{"", core.CategoryApps}) {
		t.Errorf("last search %+v", f.lastSearch())
	}
	if info, _ := tt.AccessByRole(ui.RoleTab, "Apps"); !info.Selected {
		t.Error("Apps is not the selected tab")
	}
	tt.Key(ui.Shift, ui.KeyTab)
	tt.Key(ui.Shift, ui.KeyTab)
	if f.lastSearch() != (searched{"", core.CategorySystem}) {
		t.Errorf("Shift+Tab from All went to %+v", f.lastSearch())
	}
}

func TestShowsTheTabsThatSettingsShowsInItsOrder(t *testing.T) {
	f := newFake(always())
	f.settings.Tabs = []core.Category{core.CategoryEmoji, core.CategoryApps}
	m, tt := open(t, f)
	for _, name := range []string{"All", "Emoji", "Apps"} {
		if _, ok := tt.AccessByRole(ui.RoleTab, name); !ok {
			t.Errorf("no tab %q", name)
		}
	}
	if _, ok := tt.AccessByRole(ui.RoleTab, "Files"); ok {
		t.Error("a hidden tab shows")
	}
	for _, want := range []core.Category{core.CategoryEmoji, core.CategoryApps, core.CategoryAll} {
		tt.Key(0, ui.KeyTab)
		if f.lastSearch().category != want {
			t.Errorf("Tab went to %q, want %q", f.lastSearch().category, want)
		}
	}
	// A hidden tab opened on purpose shows while it is open.
	m.Show(core.CategoryClipboard)
	tt.Frame()
	if info, ok := tt.AccessByRole(ui.RoleTab, "Clipboard"); !ok || !info.Selected {
		t.Errorf("Clipboard tab %+v %v", info, ok)
	}
}

func TestDoesNothingOnTabWhenOnlyAllShows(t *testing.T) {
	f := newFake(always())
	f.settings.Tabs = nil
	_, tt := open(t, f)
	if slices.Contains(tt.Texts(), "next") {
		t.Errorf("the Tab hint shows with one tab: %q", tt.Texts())
	}
	searches := len(f.searches)
	tt.Key(0, ui.KeyTab)
	if len(f.searches) != searches || !tt.Focused("Search") {
		t.Errorf("searches %d, want %d; search focused %v", len(f.searches), searches, tt.Focused("Search"))
	}
}

func TestClickingATabSearchesItAndKeepsTheFocus(t *testing.T) {
	f := newFake(always())
	_, tt := open(t, f)
	click(t, tt, "Snippets")
	if f.lastSearch() != (searched{"", core.CategorySnippets}) || !tt.Focused("Search") {
		t.Errorf("last search %+v, search focused %v", f.lastSearch(), tt.Focused("Search"))
	}
	if !tt.HasText("No snippets yet") {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestOpensTheActionsMenuWithModK(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	if !tt.Focused("Search actions") {
		t.Fatal("the menu's filter has no focus")
	}
	tt.Type("show")
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{safari.Actions[1].Action, ""})
}

func TestCentersTheLauncherFromTheActionsMenu(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	tt.Type("center")
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f, ran{core.CenterLauncher{}, ""})
}

func TestSaysWhenNoActionMatches(t *testing.T) {
	f := newFake(always(safari))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	tt.Type("zzz")
	if info, ok := tt.AccessByRole(ui.RoleStatus, ""); !ok || info.Name != "No matching actions" {
		t.Errorf("status %+v %v", info, ok)
	}
	tt.Key(0, ui.KeyEnter)
	wantRuns(t, f)
}

func TestKeepsTheMenuAsItOpenedAndClosesItOnATabClick(t *testing.T) {
	results := []core.Result{safari}
	f := newFake(func(string, core.Category) []core.Result { return results })
	m, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	results = []core.Result{clip}
	m.Stale()
	tt.Frame()
	if _, ok := tt.AccessByRole(ui.RoleOption, "Show in Finder"); !ok {
		t.Error("the menu lost the actions it opened with")
	}
	if _, ok := tt.AccessByRole(ui.RoleOption, "Delete"); ok {
		t.Error("the menu took the new result's actions")
	}
	click(t, tt, "Clipboard")
	if m.menu != nil || m.category != core.CategoryClipboard || !tt.Focused("Search") {
		t.Errorf("menu %v, category %q, search focused %v", m.menu != nil, m.category, tt.Focused("Search"))
	}
}

func TestTheActionsButtonOpensAndClosesTheMenu(t *testing.T) {
	f := newFake(always(safari))
	m, tt := open(t, f)
	click(t, tt, "Actions")
	if m.menu == nil {
		t.Fatal("the Actions button opened no menu")
	}
	click(t, tt, "Actions")
	if m.menu != nil {
		t.Fatal("the Actions button left the menu open")
	}
}

func TestKeepsTheMenuOpenWhileModKIsHeld(t *testing.T) {
	f := newFake(always(safari))
	m, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	tt.KeyRepeat(ui.Cmd, ui.KeyK)
	if m.menu == nil {
		t.Fatal("a held Mod+K closed the menu")
	}
	tt.Key(ui.Cmd, ui.KeyK)
	if m.menu != nil || !tt.Focused("Search") {
		t.Errorf("menu %v, search focused %v", m.menu != nil, tt.Focused("Search"))
	}
}

func TestNamesExchangeRatesInRefreshOnlyWhenTheyAreOn(t *testing.T) {
	f := newFake(always(safari))
	f.settings.CurrencyRates = true
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	if _, ok := tt.AccessByRole(ui.RoleOption, "Refresh Apps, Files, and Rates"); !ok {
		t.Errorf("texts %q", tt.Texts())
	}
	tt.Key(0, ui.KeyEscape)
	f.settings.CurrencyRates = false
	tt.Key(ui.Cmd, ui.KeyK)
	if _, ok := tt.AccessByRole(ui.RoleOption, "Refresh Apps and Files"); !ok {
		t.Errorf("texts %q", tt.Texts())
	}
}

func TestStartsOverWhenTheLauncherOpens(t *testing.T) {
	f := newFake(always())
	m, tt := open(t, f)
	tt.Type("old")
	tt.Key(ui.Cmd, ui.KeyK)
	m.Show(core.CategoryClipboard)
	tt.Frame()
	if f.lastSearch() != (searched{"", core.CategoryClipboard}) || m.query != "" || m.menu != nil || !tt.Focused("Search") {
		t.Errorf("last search %+v, query %q, menu %v", f.lastSearch(), m.query, m.menu != nil)
	}
}

func TestKeepsTheSelectionWhenResultsRefresh(t *testing.T) {
	f := newFake(always(safari, app("Notes")))
	m, tt := open(t, f)
	tt.Key(0, ui.KeyDown)
	m.Stale()
	tt.Frame()
	if got := selectedRow(tt, "Safari", "Notes"); got != "Notes" {
		t.Errorf("selected %q", got)
	}
}

func TestKeepsThePositionWhenTheSelectedEntryDisappears(t *testing.T) {
	entries := []core.Result{app("Entry 3"), app("Entry 2"), app("Entry 1")}
	f := newFake(func(string, core.Category) []core.Result { return entries })
	m, tt := open(t, f)
	tt.Key(0, ui.KeyDown)
	entries = []core.Result{entries[0], entries[2]}
	m.Stale()
	tt.Frame()
	if got := selectedRow(tt, "Entry 3", "Entry 1"); got != "Entry 1" {
		t.Errorf("selected %q", got)
	}
}

func TestOffersToTurnOnClipboardHistory(t *testing.T) {
	f := newFake(always(clip))
	f.settings.ClipboardHistory = false
	m, tt := open(t, f)
	click(t, tt, "Clipboard")
	if !tt.HasText("Clipboard history is off") || tt.HasText("Copied text") {
		t.Fatalf("texts %q", tt.Texts())
	}
	// Keys must not act on the hidden entries.
	tt.Key(ui.Cmd, ui.Key1)
	tt.Key(ui.Cmd, ui.KeyEnter)
	wantRuns(t, f)
	click(t, tt, "Turn On Clipboard History")
	if f.enabled != 1 {
		t.Fatalf("enabled %d times", f.enabled)
	}
	// Enter turns it on too, and says why it did not.
	f.enableErr = errors.New("Could not save settings.")
	m.input.Focus()
	tt.Frame()
	tt.Key(0, ui.KeyEnter)
	if f.enabled != 2 || !tt.HasText("Could not save settings.") {
		t.Fatalf("enabled %d, texts %q", f.enabled, tt.Texts())
	}
	m.Show(core.CategoryClipboard)
	tt.Frame()
	if tt.HasText("Could not save settings.") {
		t.Error("the failure stayed after the launcher opened again")
	}
}

func TestShowsActionErrors(t *testing.T) {
	f := newFake(always(safari))
	f.fail = func(core.Action) error { return errors.New("This app is no longer installed.") }
	m, tt := open(t, f)
	tt.Key(0, ui.KeyEnter)
	if _, ok := tt.AccessByRole(ui.RoleAlert, ""); !ok || !tt.HasText("This app is no longer installed.") {
		t.Fatalf("no alert: %q", tt.Texts())
	}
	// The next edit clears it.
	tt.Type("s")
	if m.notice != "" || tt.HasText("This app is no longer installed.") {
		t.Error("the error stayed after an edit")
	}
}

func TestDeletesAHistoryEntryWithModBackspace(t *testing.T) {
	calculation := core.Result{ID: "calc-history:3", Kind: core.KindCalculation, Title: "12 * 8 = 96",
		Actions: []core.ResultAction{
			core.Do("Copy Result", core.CopySavedCalculation{ID: 3}),
			core.Do("Delete from History", core.DeleteCalculation{ID: 3}),
		}}
	f := newFake(always(calculation))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyBackspace)
	wantRuns(t, f, ran{core.DeleteCalculation{ID: 3}, ""})
}

func TestLeavesModBackspaceToTheFieldWithNothingToDelete(t *testing.T) {
	f := newFake(always(safari))
	m, tt := open(t, f)
	tt.Type("hello world")
	tt.Key(ui.Cmd, ui.KeyBackspace)
	if m.query == "hello world" || len(f.runs) != 0 {
		t.Errorf("query %q, runs %+v", m.query, f.runs)
	}
}

func TestShowsModBackspaceAsTheKeyForDeletingAClipboardEntry(t *testing.T) {
	f := newFake(always(clip))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.KeyK)
	r, ok := tt.Find("Delete")
	if !ok {
		t.Fatalf("texts %q", tt.Texts())
	}
	caps, _ := tt.Find("⌫")
	if caps.Y < r.Y-8 || caps.Y > r.Y+r.H+8 {
		t.Errorf("⌫ at %+v, Delete at %+v", caps, r)
	}
}

func TestClearsTheHistoryFromTheClipboardTabAfterAsking(t *testing.T) {
	f := newFake(func(_ string, category core.Category) []core.Result {
		if category == core.CategoryClipboard {
			return []core.Result{clip}
		}
		return []core.Result{safari}
	})
	_, tt := open(t, f)
	if tt.HasText("Clear History…") {
		t.Error("Clear History shows outside the Clipboard tab")
	}
	click(t, tt, "Clipboard")
	click(t, tt, "Clear History…")
	if !tt.HasText("Delete all clipboard history except pinned entries?") {
		t.Fatalf("texts %q", tt.Texts())
	}
	pressButton(t, tt, "Clear Clipboard History")
	wantRuns(t, f, ran{core.ClearClipboard{}, ""})
	// Mod+K lists it there too.
	tt.Key(ui.Cmd, ui.KeyK)
	if _, ok := tt.AccessByRole(ui.RoleOption, "Clear Clipboard History"); !ok {
		t.Error("the menu has no Clear Clipboard History")
	}
}

// Escape closes the menu, then hides the launcher, as docs/features.md
// says; it never clears the query.
func TestEscapeClosesTheMenuThenHides(t *testing.T) {
	f := newFake(always(safari))
	m, tt := open(t, f)
	tt.Type("saf")
	tt.Key(ui.Cmd, ui.KeyK)
	tt.Key(0, ui.KeyEscape)
	if m.menu != nil || m.query != "saf" || f.hidden != 0 {
		t.Fatalf("menu %v, query %q, hidden %d", m.menu != nil, m.query, f.hidden)
	}
	tt.KeyRepeat(0, ui.KeyEscape)
	if f.hidden != 0 {
		t.Fatal("a held Escape hid the launcher")
	}
	tt.Key(0, ui.KeyEscape)
	if f.hidden != 1 || m.query != "saf" {
		t.Errorf("hidden %d, query %q", f.hidden, m.query)
	}
}

func TestRunsTheRowThatWasClicked(t *testing.T) {
	notes := app("Notes")
	f := newFake(always(safari, notes))
	_, tt := open(t, f)
	click(t, tt, "Notes")
	wantRuns(t, f, ran{notes.Actions[0].Action, notes.ID})
	if !tt.Focused("Search") {
		t.Error("the click took the focus from the search field")
	}
}

func TestRunsARowWithModAndItsDigit(t *testing.T) {
	notes := app("Notes")
	f := newFake(always(safari, notes))
	_, tt := open(t, f)
	tt.Key(ui.Cmd, ui.Key2)
	tt.Key(ui.Cmd, ui.Key3)
	wantRuns(t, f, ran{notes.Actions[0].Action, notes.ID})
}

func TestOpensTheActionsOfARowOnRightClick(t *testing.T) {
	f := newFake(always(safari, restart))
	m, tt := open(t, f)
	r, ok := tt.Find("Restart")
	if !ok {
		t.Fatal("no Restart row")
	}
	tt.RightClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	if selectedRow(tt, "Safari", "Restart") != "Restart" || m.menu == nil {
		t.Fatalf("selected %q, menu %v", selectedRow(tt, "Safari", "Restart"), m.menu != nil)
	}
	if _, ok := tt.AccessByRole(ui.RoleOption, "Run"); !ok {
		t.Error("the menu has no Run")
	}
}

func TestMarksPinnedResults(t *testing.T) {
	pinned := safari
	pinned.Pinned = true
	f := newFake(always(pinned))
	_, tt := open(t, f)
	if _, ok := tt.Access("Pinned"); !ok {
		t.Error("no pinned mark")
	}
}

func TestFollowsTheDesktopsAppearance(t *testing.T) {
	f := newFake(always())
	_, tt := open(t, f)
	for _, c := range []struct {
		dark bool
		want ui.Color
	}{{false, kit.Light.Paper}, {true, kit.Dark.Paper}, {false, kit.Light.Paper}} {
		tt.SetDark(c.dark)
		got := tt.Image().RGBAAt(2, 300)
		if got.R != c.want.R || got.G != c.want.G || got.B != c.want.B {
			t.Errorf("dark %v: background %v, want %v", c.dark, got, c.want)
		}
	}
}
