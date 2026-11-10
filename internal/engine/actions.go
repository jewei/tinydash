package engine

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/calc"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/files"
	"github.com/jewei/tinydash/internal/feature/library"
	"github.com/jewei/tinydash/internal/search"
	"github.com/jewei/tinydash/internal/store"
)

// OS is what actions do outside TinyDash. The app passes mygo and
// internal/platform. Its errors reach the user, so they are sentences.
type OS interface {
	LaunchApp(path string) error
	// OpenPath opens a file or folder with its default app.
	OpenPath(path string) error
	// Reveal shows a file or app in the file manager.
	Reveal(path string) error
	// OpenURL opens a web or mail address with the app the OS assigns.
	OpenURL(url string) error
	// WriteText puts text on the clipboard. A secret asks clipboard
	// managers not to keep it.
	WriteText(text string, secret bool) error
	// WriteClip puts a saved clipboard entry back on the clipboard.
	WriteClip(content clipboard.Content) error
	// ClipboardText is the clipboard's text. ok is false when it holds
	// none, and, unless secretToo, when its text is secret, such as a
	// password that a password manager copied.
	ClipboardText(secretToo bool) (text string, ok bool)
	RunSystem(command core.SystemCommand) error
	// OpenCalendarApp opens the app that holds an Up Next item: Calendar
	// for an event, Reminders for a reminder.
	OpenCalendarApp(kind core.UpNextKind) error
}

// Shell is what actions do to TinyDash's own windows and settings.
type Shell interface {
	// Hide hides the launcher without moving focus.
	Hide() error
	// Dismiss hides the launcher and gives focus back to the app the user
	// came from, so a copy can be pasted at once.
	Dismiss() error
	// ShowAgain shows the launcher as it was, without a reset, so the
	// error of an action that hid it shows where the user ran it.
	ShowAgain() error
	// ShowHUD shows a short message, such as "Copied", that never takes
	// focus.
	ShowHUD(text string)
	OpenSettings() error
	// CenterLauncher forgets where the launcher was dragged and centers
	// it. It saves the settings.
	CenterLauncher() error
	// TypeQuery replaces the launcher's query.
	TypeQuery(text string)
	// Refresh rescans the apps and files and downloads the rates again.
	Refresh()
	// TurnOnCurrencyRates saves the setting and downloads the rates.
	TurnOnCurrencyRates() error
	// Quit ends TinyDash.
	Quit()
	// AskCalendarAccess asks the OS to let Up Next read calendars and
	// reminders, and loads the widgets again once the user answers. It
	// returns at once.
	AskCalendarAccess()
}

// Origin is where the user ran an action, which decides where its error
// shows.
type Origin uint8

const (
	// FromLauncher: the launcher shows the error under the query. Settings
	// runs its actions this way too and shows their errors itself.
	FromLauncher Origin = iota
	// FromHotkey: the user pressed a hotkey in another app. RunHotkey
	// returns the error, and the app shows it in a system notification.
	FromHotkey
)

// Actions runs what results offer. Each action is checked again before it
// runs, against the published state, so a view can only open what TinyDash
// itself indexed or offered. Every change of state goes through the writer.
type Actions struct {
	engine  *Engine
	host    OS
	shell   Shell
	hotkeys HotkeyRegistry
	// now is the clock in the user's time zone, which snippet and
	// quicklink dates read.
	now func() time.Time

	mu sync.Mutex
	// found are the paths that Spotlight searches returned.
	found files.FoundPaths
}

// NewActions returns the actions of e.
func NewActions(e *Engine, host OS, shell Shell, hotkeys HotkeyRegistry, now func() time.Time) *Actions {
	return &Actions{engine: e, host: host, shell: shell, hotkeys: hotkeys, now: now}
}

// RememberFound adds a path that a Spotlight search returned, so Open and
// Reveal accept it as they accept indexed ones.
func (a *Actions) RememberFound(path string) {
	a.mu.Lock()
	defer a.mu.Unlock()
	a.found.Remember(path)
}

// Offered reports whether the launcher offered this file: it is indexed,
// or Spotlight returned it. Opening and previewing both check this.
func (a *Actions) Offered(path string) bool {
	return a.offered(a.state(), path)
}

func (a *Actions) offered(s *search.Snapshot, path string) bool {
	if s.Files.Contains(path) {
		return true
	}
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.found.Contains(path)
}

// Run runs an action that a result or the actions menu gave. id is the
// result it came from, for ranking, or empty. Run waits for the writer and
// the OS, so it must not run on the main thread.
func (a *Actions) Run(origin Origin, action core.Action, id core.ID) error {
	if err := action.Dispatch(handler{a, origin}); err != nil {
		return err
	}
	if action.CountsAsUse() && learnsFromUse(id) {
		a.recordUse(id)
	}
	// Quit ends the process, so it goes last, once the use is saved.
	if _, quit := action.(core.Quit); quit {
		a.shell.Quit()
	}
	return nil
}

// RunHotkey runs the main action of the result that a hotkey belongs to,
// as Enter would in the launcher.
func (a *Actions) RunHotkey(id core.ID) error {
	action, err := hotkeyAction(a.state(), id)
	if err != nil {
		return err
	}
	return a.Run(FromHotkey, action, id)
}

func (a *Actions) state() *search.Snapshot { return a.engine.Snapshot(a.now()) }

// change runs apply on the writer and waits for it.
func (a *Actions) change(apply func(next *search.Snapshot, db *store.Store) error) error {
	return <-a.engine.Change(apply)
}

// afterWrite wraps an error that comes after a database write succeeded.
// The change made so far is published, so the lists in memory stay in step
// with the database.
type afterWrite struct{ err error }

func (e afterWrite) Error() string { return e.err.Error() }

// write is change for a change that writes the database in more than one
// step; see afterWrite.
func (a *Actions) write(apply func(next *search.Snapshot, db *store.Store) error) error {
	var late error
	err := a.change(func(next *search.Snapshot, db *store.Store) error {
		err := apply(next, db)
		if after, ok := err.(afterWrite); ok {
			late = after.err
			return nil
		}
		return err
	})
	if err != nil {
		return err
	}
	return late
}

// learnsFromUse: results with stable IDs learn from use; computed answers
// do not.
func learnsFromUse(id core.ID) bool {
	traits, ok := id.Traits()
	return ok && traits.LearnsFromUse
}

// recordUse ranks id higher. The action already worked, so a failed save
// only costs the ranking after a restart.
func (a *Actions) recordUse(id core.ID) {
	now := a.now().Unix()
	err := a.change(func(next *search.Snapshot, db *store.Store) error {
		usage, used := next.Usage.Record(id, now)
		next.Usage = usage
		if err := db.RecordUse(context.Background(), id, used); err != nil {
			slog.Warn("Could not save usage", "error", err)
		}
		return nil
	})
	if err != nil {
		slog.Warn("Could not record usage", "error", err)
	}
}

// handler runs one action for Run.
type handler struct {
	*Actions
	origin Origin
}

var _ core.Handler = handler{}

func (h handler) Launch(a core.Launch) error {
	if !h.state().Apps.Contains(a.Path) {
		return errors.New("This app is no longer installed.")
	}
	if err := h.host.LaunchApp(a.Path); err != nil {
		return err
	}
	return h.shell.Hide()
}

func (h handler) Open(a core.Open) error {
	if !h.Offered(a.Path) {
		return errors.New("This file is not in the index.")
	}
	if err := h.openPath(a.Path); err != nil {
		return err
	}
	return h.shell.Hide()
}

func (h handler) Reveal(a core.Reveal) error {
	s := h.state()
	if !s.Apps.Contains(a.Path) && !h.offered(s, a.Path) {
		return errors.New("This item is not in the index.")
	}
	if err := h.host.Reveal(a.Path); err != nil {
		return fmt.Errorf("Could not show it in %s: %v. Check that it still exists.", fileManager, err)
	}
	return h.shell.Hide()
}

func (h handler) OpenURL(a core.OpenURL) error {
	if !webAddress(a.URL) {
		return errors.New("Only web addresses can be opened.")
	}
	if err := h.openURL(a.URL); err != nil {
		return err
	}
	return h.shell.Hide()
}

func (h handler) Copy(a core.Copy) error {
	return h.copyAndClose(func() error { return h.host.WriteText(a.Text, false) })
}

func (h handler) CopySecret(a core.CopySecret) error {
	return h.copyAndClose(func() error { return h.host.WriteText(a.Text, true) })
}

// CopyCalculation computes the answer again, so the history keeps only
// what the calculator itself answers.
func (h handler) CopyCalculation(a core.CopyCalculation) error {
	value, ok := calc.Result(a.Query, rates(h.state()))
	if !ok {
		return errors.New("This calculation no longer has an answer.")
	}
	if err := h.copyAndClose(func() error { return h.host.WriteText(value, false) }); err != nil {
		return err
	}
	h.rememberCalculation(a.Query, value)
	return nil
}

func (h handler) CopySavedCalculation(a core.CopySavedCalculation) error {
	entry, ok := h.state().Calculations.Get(a.ID)
	if !ok {
		return errors.New("This calculation is no longer in the history.")
	}
	if err := h.copyAndClose(func() error { return h.host.WriteText(entry.Result, false) }); err != nil {
		return err
	}
	h.rememberCalculation(entry.Expression, entry.Result)
	return nil
}

func (h handler) DeleteCalculation(a core.DeleteCalculation) error {
	return h.change(func(next *search.Snapshot, db *store.Store) error {
		ctx := context.Background()
		if err := db.DeleteCalculation(ctx, a.ID); err != nil {
			return err
		}
		return reloadCalculations(ctx, next, db)
	})
}

func (h handler) ClearCalculations(core.ClearCalculations) error {
	return h.change(func(next *search.Snapshot, db *store.Store) error {
		ctx := context.Background()
		if err := db.ClearCalculations(ctx); err != nil {
			return err
		}
		return reloadCalculations(ctx, next, db)
	})
}

func (h handler) CopyClip(a core.CopyClip) error {
	content, err := h.clip(a.ID)
	if err != nil {
		return err
	}
	return h.copyAndClose(func() error { return h.host.WriteClip(content) })
}

func (h handler) CopyClipText(a core.CopyClipText) error {
	content, err := h.clip(a.ID)
	if err != nil {
		return err
	}
	text, ok := plainText(content)
	if !ok {
		return errors.New("An image has no text to copy.")
	}
	return h.copyAndClose(func() error { return h.host.WriteText(text, false) })
}

// DeleteClip works while history is off too: deleting reveals nothing.
func (h handler) DeleteClip(a core.DeleteClip) error {
	return h.write(func(next *search.Snapshot, db *store.Store) error {
		ctx := context.Background()
		if err := db.DeleteClip(ctx, a.ID); err != nil {
			return err
		}
		next.Pins = next.Pins.Remove(clipID(a.ID))
		// Dropped here too, without reading the history back, so a failed
		// read cannot keep it on screen.
		next.Clipboard = next.Clipboard.Keep(func(entry clipboard.Entry) bool { return entry.ID != a.ID })
		return nil
	})
}

// CopyPlainText reads as widgets do, so a secret copy stays as it is; any
// copy it can read is one that history and links may use too.
func (h handler) CopyPlainText(core.CopyPlainText) error {
	text, ok := h.host.ClipboardText(false)
	if !ok {
		return errors.New("The clipboard holds no text, or its text is secret, such as a copied password.")
	}
	return h.copyAndClose(func() error { return h.host.WriteText(text, false) })
}

func (h handler) ClearClipboard(core.ClearClipboard) error {
	return <-h.engine.ClearClipboard()
}

func (h handler) CopySnippet(a core.CopySnippet) error {
	s := h.state()
	item, ok := s.Library.Find(a.ID)
	if !ok || item.Kind != library.KindSnippet {
		return errors.New("This snippet was deleted.")
	}
	// The clipboard may hold a password, so text that includes it is
	// copied as secret and stays out of clipboard histories.
	readClipboard := false
	text := library.RenderSnippet(item.Text, a.Query, s.Now, func() string {
		readClipboard = true
		text, _ := h.host.ClipboardText(true)
		return text
	})
	return h.copyAndClose(func() error { return h.host.WriteText(text, readClipboard) })
}

func (h handler) OpenQuicklink(a core.OpenQuicklink) error {
	s := h.state()
	item, ok := s.Library.Find(a.ID)
	if !ok || item.Kind != library.KindQuicklink {
		return errors.New("This quicklink was deleted.")
	}
	// Read the way widgets read it: a secret copy never goes into a link.
	target, err := library.QuicklinkTarget(item.Text, a.Query, s.Now, s.Home, func() string {
		text, _ := h.host.ClipboardText(false)
		return text
	})
	if err != nil {
		return err
	}
	if target.URL != "" {
		err = h.openURL(target.URL)
	} else {
		err = h.openPath(target.Path)
	}
	if err != nil {
		return err
	}
	return h.shell.Hide()
}

// System hides the launcher first: Lock and Sleep must not leave it on
// screen. When a command run in the launcher fails, the launcher shows
// again as it was, so the error shows where the user ran it. A hotkey's
// error goes to a notification, and the launcher must not come up over the
// app the user is in.
func (h handler) System(a core.System) error {
	if err := h.shell.Hide(); err != nil {
		return err
	}
	err := h.host.RunSystem(a.Command)
	if err == nil {
		return nil
	}
	if h.origin == FromLauncher {
		if shown := h.shell.ShowAgain(); shown != nil {
			return shown
		}
	}
	return err
}

func (h handler) Unpin(a core.Unpin) error {
	return h.change(func(next *search.Snapshot, db *store.Store) error {
		if err := db.SetPinned(context.Background(), a.ID, false); err != nil {
			return err
		}
		next.Pins = next.Pins.Remove(a.ID)
		return nil
	})
}

func (h handler) TurnOnCurrencyRates(core.TurnOnCurrencyRates) error {
	return h.shell.TurnOnCurrencyRates()
}

func (h handler) Refresh(core.Refresh) error {
	h.shell.Refresh()
	return nil
}

func (h handler) OpenSettings(core.OpenSettings) error { return h.shell.OpenSettings() }

func (h handler) CenterLauncher(core.CenterLauncher) error { return h.shell.CenterLauncher() }

func (h handler) TypeQuery(a core.TypeQuery) error {
	if strings.TrimSpace(a.Text) == "" || utf8.RuneCountInString(a.Text) > library.MaxTyped {
		return errors.New("This keyword cannot be typed. Type it yourself.")
	}
	h.shell.TypeQuery(a.Text)
	return nil
}

// Quit does its work in Run, after the use is saved.
func (h handler) Quit(core.Quit) error { return nil }

func (h handler) OpenUpNext(a core.OpenUpNext) error {
	if a.Kind != core.UpNextEvent && a.Kind != core.UpNextReminder {
		return errors.New("This item cannot be opened.")
	}
	if err := h.host.OpenCalendarApp(a.Kind); err != nil {
		return err
	}
	return h.shell.Hide()
}

// AskCalendarAccess asks only while Up Next is on, as only its widget
// offers it.
func (h handler) AskCalendarAccess(core.AskCalendarAccess) error {
	if !h.state().Settings.ShowUpNext {
		return errors.New("Turn on Up Next in Settings > Widgets first.")
	}
	h.shell.AskCalendarAccess()
	return nil
}

// copyAndClose copies, then hides the launcher and hands focus back so the
// user can paste right away, and says that it worked.
func (a *Actions) copyAndClose(copy func() error) error {
	if err := copy(); err != nil {
		return err
	}
	if err := a.shell.Dismiss(); err != nil {
		return err
	}
	a.shell.ShowHUD("Copied")
	return nil
}

// rememberCalculation keeps a copied calculation in the history. The copy
// already worked, so a failed save only costs the history entry.
func (a *Actions) rememberCalculation(expression, value string) {
	if utf8.RuneCountInString(expression) > calc.MaxExpression {
		return
	}
	now := a.now().Unix()
	err := a.change(func(next *search.Snapshot, db *store.Store) error {
		ctx := context.Background()
		if err := db.SaveCalculation(ctx, expression, value, now); err != nil {
			return err
		}
		return reloadCalculations(ctx, next, db)
	})
	if err != nil {
		slog.Warn("Could not save the calculation", "error", err)
	}
}

// clip is the full content of a saved clipboard entry, hidden like search
// results while history is off.
func (a *Actions) clip(id int64) (clipboard.Content, error) {
	gone := errors.New("This entry is no longer in the history.")
	if !a.state().Settings.ClipboardHistoryEnabled {
		return clipboard.Content{}, gone
	}
	content, ok, err := a.engine.store.Clip(context.Background(), id)
	if err != nil {
		return clipboard.Content{}, err
	}
	if !ok {
		return clipboard.Content{}, gone
	}
	return content, nil
}

func (a *Actions) openPath(path string) error {
	if _, err := os.Stat(path); err != nil {
		return fmt.Errorf("%s no longer exists.", path)
	}
	if err := a.host.OpenPath(path); err != nil {
		return fmt.Errorf("Could not open %s: %v. Check that an app on this computer opens this kind of file.", path, err)
	}
	return nil
}

func (a *Actions) openURL(url string) error {
	if err := a.host.OpenURL(url); err != nil {
		return fmt.Errorf("Could not open %s: %v. Check that a default browser or mail app is set.", url, err)
	}
	return nil
}

// webAddress reports whether url is an http or https address, the only
// kind the OS opener may get: another scheme can start a program.
func webAddress(url string) bool {
	scheme, _, ok := strings.Cut(url, ":")
	if !ok {
		return false
	}
	scheme = strings.ToLower(scheme)
	return scheme == "http" || scheme == "https"
}

// plainText is the text of a clipboard entry: its text, or one path per
// line. An image has none.
func plainText(c clipboard.Content) (string, bool) {
	switch c.Kind {
	case clipboard.KindText:
		return c.Text, true
	case clipboard.KindFiles:
		return strings.Join(c.Files, "\n"), true
	}
	return "", false
}

// rates is the snapshot's rate table for the calculator: a nil
// *currency.Rates in the interface would not be a nil calc.Rates.
func rates(s *search.Snapshot) calc.Rates {
	if s.Rates == nil {
		return nil
	}
	return *s.Rates
}

func reloadCalculations(ctx context.Context, next *search.Snapshot, db *store.Store) error {
	history, err := calculations(ctx, db)
	if err != nil {
		return err
	}
	next.Calculations = history
	return nil
}

func calculations(ctx context.Context, db *store.Store) (calc.History, error) {
	saved, err := db.Calculations(ctx)
	if err != nil {
		return calc.History{}, err
	}
	entries := make([]calc.Calculation, len(saved))
	for i, c := range saved {
		entries[i] = calc.Calculation{ID: c.ID, Expression: c.Expression, Result: c.Result}
	}
	return calc.NewHistory(entries), nil
}
