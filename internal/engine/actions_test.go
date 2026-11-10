package engine

import (
	"context"
	"os"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// withLibrary saves a snippet (ID 1), a web quicklink (ID 2), and a folder
// quicklink (ID 3).
func withLibrary(first *search.Snapshot, db *store.Store) {
	ctx := context.Background()
	for _, item := range []library.Item{
		{Kind: library.KindSnippet, Name: "Sig", Text: "Regards, {clipboard}"},
		{Kind: library.KindQuicklink, Name: "Search", Keyword: "s", Text: "https://x.test/?q={query}"},
		{Kind: library.KindQuicklink, Name: "Docs", Keyword: "d", Text: "~/Documents/{query}"},
	} {
		if _, err := db.SaveLibraryItem(ctx, item); err != nil {
			panic(err)
		}
	}
	items, err := db.Library(ctx)
	if err != nil {
		panic(err)
	}
	first.Library = library.NewIndex(items)
}

func TestActionsForWhatTinyDashDidNotOfferNeverReachTheOS(t *testing.T) {
	w := newWorld(t, withLibrary)
	notes := golden.Abs("/Users/me/notes.txt")
	for _, c := range []struct {
		action core.Action
		want   string
	}{
		{core.Launch{Path: golden.Abs("/Applications/Gone.app")}, "This app is no longer installed."},
		{core.Open{Path: notes}, "This file is not in the index."},
		{core.Open{Path: safari}, "This file is not in the index."},
		{core.Reveal{Path: notes}, "This item is not in the index."},
		{core.OpenURL{URL: "javascript:alert(1)"}, "Only web addresses can be opened."},
		{core.OpenURL{URL: "file:///etc/passwd"}, "Only web addresses can be opened."},
		{core.CopyClip{ID: 99}, "This entry is no longer in the history."},
		{core.CopyClipText{ID: 99}, "This entry is no longer in the history."},
		{core.CopySnippet{ID: 99}, "This snippet was deleted."},
		{core.CopySnippet{ID: 2}, "This snippet was deleted."},
		{core.OpenQuicklink{ID: 1}, "This quicklink was deleted."},
		{core.OpenQuicklink{ID: 3, Query: "../x"}, "A path quicklink accepts a name, not a path."},
		{core.CopyCalculation{Query: "hello"}, "This calculation no longer has an answer."},
		{core.CopySavedCalculation{ID: 5}, "This calculation is no longer in the history."},
		{core.TypeQuery{Text: "  "}, "This keyword cannot be typed. Type it yourself."},
		{core.TypeQuery{Text: strings.Repeat("x", library.MaxTyped+1)}, "This keyword cannot be typed. Type it yourself."},
		{core.OpenUpNext{Kind: "calendar"}, "This item cannot be opened."},
		{core.AskCalendarAccess{}, "Turn on Up Next in Settings > Widgets first."},
	} {
		err := w.run(c.action, core.SourceApp.ID(safari))
		if errorText(err) != c.want {
			t.Errorf("%#v: %v, want %q", c.action, err, c.want)
		}
		wantSteps(t, w)
	}
	if bonus := w.snapshot().Usage.Bonus(core.SourceApp.ID(safari), testNow.Unix()); bonus != 0 {
		t.Errorf("a refused action counted as use: %d", bonus)
	}
}

func TestOfferedItemsRun(t *testing.T) {
	dir := t.TempDir()
	notes := filepath.Join(dir, "notes.txt")
	if err := os.WriteFile(notes, []byte("hi"), 0o600); err != nil {
		t.Fatal(err)
	}
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.Settings.FileSearchFolders = []string{dir}
	})
	w.engine.RebuildFiles(FileDisk{Disk: osDisk{}})(context.Background())

	steps := []struct {
		action core.Action
		want   []string
	}{
		{core.Launch{Path: safari}, []string{"launch /Applications/Safari.app", "hide"}},
		{core.Reveal{Path: safari}, []string{"reveal /Applications/Safari.app", "hide"}},
		{core.Open{Path: notes}, []string{"open " + golden.Slash(notes), "hide"}},
		{core.Reveal{Path: notes}, []string{"reveal " + golden.Slash(notes), "hide"}},
		{core.OpenURL{URL: "HTTPS://x.test"}, []string{"open HTTPS://x.test", "hide"}},
		{core.Copy{Text: "hi"}, []string{"copy hi", "dismiss", "hud Copied"}},
		{core.CopySecret{Text: "pw"}, []string{"copy secret pw", "dismiss", "hud Copied"}},
		{core.TypeQuery{Text: "jira "}, []string{"type jira "}},
		{core.Refresh{}, []string{"refresh"}},
		{core.OpenSettings{}, []string{"settings"}},
		{core.CenterLauncher{}, []string{"center"}},
		{core.TurnOnCurrencyRates{}, []string{"rates on"}},
		{core.OpenUpNext{Kind: core.UpNextEvent}, []string{"open up next event", "hide"}},
		{core.OpenUpNext{Kind: core.UpNextReminder}, []string{"open up next reminder", "hide"}},
	}
	for _, s := range steps {
		if err := w.run(s.action, ""); err != nil {
			t.Fatalf("%#v: %v", s.action, err)
		}
		wantSteps(t, w, s.want...)
	}

	// The file went after the scan: the OS opener never gets it.
	if err := os.Remove(notes); err != nil {
		t.Fatal(err)
	}
	if err := w.run(core.Open{Path: notes}, ""); errorText(err) != notes+" no longer exists." {
		t.Errorf("open a gone file: %v", err)
	}
	wantSteps(t, w)
}

// The widget's Allow Access button asks; with Up Next off, nothing does.
func TestAsksForCalendarAccessWhileUpNextIsOn(t *testing.T) {
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) { first.Settings.ShowUpNext = true })
	if err := w.run(core.AskCalendarAccess{}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "ask calendar access")
}

func TestOSErrorsSayWhatToDo(t *testing.T) {
	w := newWorld(t, nil)
	w.host.fail = errNoDarkMode
	err := w.run(core.OpenURL{URL: "https://x.test"}, "")
	if want := "Could not open https://x.test: This desktop has no dark mode.. Check that a default browser or mail app is set."; errorText(err) != want {
		t.Errorf("%v", err)
	}
	err = w.run(core.Reveal{Path: safari}, "")
	if !strings.HasPrefix(errorText(err), "Could not show it in ") || !strings.HasSuffix(errorText(err), "Check that it still exists.") {
		t.Errorf("%v", err)
	}
	// A failed copy does not hide the launcher or say "Copied".
	if err := w.run(core.Copy{Text: "hi"}, ""); err != errNoDarkMode {
		t.Errorf("%v", err)
	}
	if got := w.steps.take(); got[len(got)-1] != "copy hi" {
		t.Errorf("steps after a failed copy: %q", got)
	}
}

func TestAFailedCommandShowsTheLauncherAgainOnlyWhenRunFromIt(t *testing.T) {
	w := newWorld(t, nil)
	command := core.System{Command: core.CommandToggleDarkMode}
	w.host.fail = errNoDarkMode
	if err := w.actions.Run(FromLauncher, command, ""); err != errNoDarkMode {
		t.Errorf("%v", err)
	}
	wantSteps(t, w, "hide", "run toggleDarkMode", "show again")
	// The launcher may be on screen behind the user's app, or hidden:
	// either way, a hotkey's failure must not bring it up.
	if err := w.actions.Run(FromHotkey, command, ""); err != errNoDarkMode {
		t.Errorf("%v", err)
	}
	wantSteps(t, w, "hide", "run toggleDarkMode")
	w.host.fail = nil
	for _, origin := range []Origin{FromLauncher, FromHotkey} {
		if err := w.actions.Run(origin, command, ""); err != nil {
			t.Fatal(err)
		}
		wantSteps(t, w, "hide", "run toggleDarkMode")
	}
}

func TestUsageIsRecordedAfterAUseOfAResultThatLearns(t *testing.T) {
	w := newWorld(t, nil)
	app := core.SourceApp.ID(safari)
	used := func(id core.ID) uint32 {
		t.Helper()
		saved, err := w.db.Usage(context.Background())
		if err != nil {
			t.Fatal(err)
		}
		return saved[id].Count
	}
	if err := w.run(core.Launch{Path: safari}, app); err != nil {
		t.Fatal(err)
	}
	if used(app) != 1 || w.snapshot().Usage.Bonus(app, testNow.Unix()) == 0 {
		t.Fatalf("a launch was not counted: %d", used(app))
	}
	// Reveal does not count as use.
	if err := w.run(core.Reveal{Path: safari}, app); err != nil {
		t.Fatal(err)
	}
	if used(app) != 1 {
		t.Errorf("a reveal counted: %d", used(app))
	}
	// Computed answers have no lasting ID.
	if err := w.run(core.Copy{Text: "2"}, "calc:1+1"); err != nil {
		t.Fatal(err)
	}
	if n := len(w.snapshot().Usage.Ranked(testNow.Unix())); n != 1 {
		t.Errorf("%d results have usage", n)
	}
}

func TestQuitSavesTheUseFirst(t *testing.T) {
	w := newWorld(t, nil)
	quit := core.SourceSystem.ID("quit-tinydash")
	if err := w.run(core.Quit{}, quit); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "quit")
	saved, err := w.db.Usage(context.Background())
	if err != nil || saved[quit].Count != 1 {
		t.Errorf("%v %v", saved, err)
	}
}

func TestOnlyStableResultsLearnFromUse(t *testing.T) {
	if !learnsFromUse(core.SourceApp.ID(safari)) {
		t.Error("apps")
	}
	for _, id := range []core.ID{"calc:1+1", "clip:3", "shell:git pull", ""} {
		if learnsFromUse(id) {
			t.Error(id)
		}
	}
}

func TestOpensOnlyWebAddresses(t *testing.T) {
	for _, url := range []string{"https://x.test", "HTTP://x.test"} {
		if !webAddress(url) {
			t.Error(url)
		}
	}
	for _, url := range []string{"javascript:alert(1)", "file:///etc/passwd", "httpx://x", " https://x", "ms-settings:", "no-scheme"} {
		if webAddress(url) {
			t.Error(url)
		}
	}
}

func TestOpensOnlyIndexedFilesOrOnesSpotlightReturned(t *testing.T) {
	w := newWorld(t, nil)
	notes, other := golden.Abs("/Users/me/notes.txt"), golden.Abs("/Users/me/other.txt")
	if w.actions.Offered(notes) {
		t.Error("offered before Spotlight returned it")
	}
	w.actions.RememberFound(notes)
	if !w.actions.Offered(notes) || w.actions.Offered(other) {
		t.Error("Spotlight's paths")
	}
	if err := w.run(core.Reveal{Path: notes}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "reveal /Users/me/notes.txt", "hide")
}

func TestCopyingAHistoryEntryMovesItToTheTop(t *testing.T) {
	w := newWorld(t, nil)
	for _, query := range []string{"1+1", "2*3"} {
		if err := w.run(core.CopyCalculation{Query: query}, ""); err != nil {
			t.Fatal(err)
		}
	}
	wantSteps(t, w, "copy 2", "dismiss", "hud Copied", "copy 6", "dismiss", "hud Copied")
	older, ok := w.snapshot().Calculations.Answer("= 1+1")
	if !ok || len(older) != 1 {
		t.Fatalf("%v", older)
	}
	id, err := strconv.ParseInt(strings.TrimPrefix(string(older[0].ID), "calc-history:"), 10, 64)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.run(core.CopySavedCalculation{ID: id}, ""); err != nil {
		t.Fatal(err)
	}
	all, _ := w.snapshot().Calculations.Answer("=")
	var titles []string
	for _, r := range all {
		titles = append(titles, r.Title)
	}
	if !slices.Equal(titles, []string{"2", "6"}) {
		t.Errorf("%q", titles)
	}

	// A delete and a clear change the history at once. The copy moved the
	// entry to the top under a new ID.
	id, err = strconv.ParseInt(strings.TrimPrefix(string(all[0].ID), "calc-history:"), 10, 64)
	if err != nil {
		t.Fatal(err)
	}
	if err := w.run(core.DeleteCalculation{ID: id}, ""); err != nil {
		t.Fatal(err)
	}
	if all, _ := w.snapshot().Calculations.Answer("="); len(all) != 1 {
		t.Errorf("after a delete: %d", len(all))
	}
	if err := w.run(core.ClearCalculations{}, ""); err != nil {
		t.Fatal(err)
	}
	if all, _ := w.snapshot().Calculations.Answer("="); len(all) != 0 {
		t.Errorf("after a clear: %d", len(all))
	}
}

func TestClipActionsUseOnlySavedEntriesWhileHistoryIsOn(t *testing.T) {
	w := newWorld(t, func(first *search.Snapshot, _ *store.Store) {
		first.Settings.ClipboardHistoryEnabled = true
	})
	report := golden.Abs("/Users/me/report.pdf")
	for _, content := range []clipboard.Content{
		clipboard.Text("hello"),
		clipboard.Files([]string{report, home}),
		clipboard.Image([]byte{1, 2, 3}, 1, 1),
	} {
		if err := <-w.engine.SaveClip(w.engine.ClipEpoch(), content, testNow.Unix()); err != nil {
			t.Fatal(err)
		}
	}
	entries, err := w.db.ClipboardHistory(context.Background())
	if err != nil || len(entries) != 3 {
		t.Fatalf("%v %v", entries, err)
	}
	image, files, text := entries[0].ID, entries[1].ID, entries[2].ID

	if err := w.run(core.CopyClip{ID: text}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, `copy text "hello"`, "dismiss", "hud Copied")
	if err := w.run(core.CopyClipText{ID: files}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "copy "+report+"\n"+home, "dismiss", "hud Copied")
	if err := w.run(core.CopyClipText{ID: image}, ""); errorText(err) != "An image has no text to copy." {
		t.Errorf("%v", err)
	}
	wantSteps(t, w)

	clip := core.SourceClip.ID(strconv.FormatInt(text, 10))
	if err := w.run(core.Pin{ID: clip}, ""); err != nil {
		t.Fatal(err)
	}
	if err := w.run(core.DeleteClip{ID: text}, ""); err != nil {
		t.Fatal(err)
	}
	if w.snapshot().Pins.Contains(clip) {
		t.Error("a deleted entry stays pinned")
	}
	if _, ok := w.snapshot().Clipboard.Get(text, clipboard.Context{}); ok {
		t.Error("a deleted entry stays in the history")
	}

	// While history is off, saved entries look gone.
	setHistory(t, w.engine, false)
	if err := w.run(core.CopyClip{ID: files}, ""); errorText(err) != "This entry is no longer in the history." {
		t.Errorf("%v", err)
	}
	wantSteps(t, w)
}

func TestTheClipboardFillsSnippetsAsSecretAndNeverFillsLinksWithASecret(t *testing.T) {
	w := newWorld(t, withLibrary)
	w.host.clipboard, w.host.secret = "hunter2", true
	if err := w.run(core.CopySnippet{ID: 1}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "read clipboard", "copy secret Regards, hunter2", "dismiss", "hud Copied")
	if err := w.run(core.CopyPlainText{}, ""); errorText(err) != "The clipboard holds no text, or its text is secret, such as a copied password." {
		t.Errorf("%v", err)
	}
	wantSteps(t, w, "read clipboard")
	if err := w.run(core.OpenQuicklink{ID: 2, Query: "a b"}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "open https://x.test/?q=a%20b", "hide")

	w.host.secret = false
	if err := w.run(core.CopyPlainText{}, ""); err != nil {
		t.Fatal(err)
	}
	wantSteps(t, w, "read clipboard", "copy hunter2", "dismiss", "hud Copied")
}

func TestClearingTheClipboardBumpsItsEpoch(t *testing.T) {
	w := newWorld(t, nil)
	epoch := w.engine.ClipEpoch()
	if err := w.run(core.ClearClipboard{}, ""); err != nil {
		t.Fatal(err)
	}
	if w.engine.ClipEpoch() == epoch {
		t.Error("a clear kept the epoch, so a capture read before it could come back")
	}
}
