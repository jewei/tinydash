package core

// Action is what a result does when chosen. The set is closed: only this
// package defines actions, and each one calls its own Handler method, so a
// new action does not compile until every Handler handles it.
type Action interface {
	// Dispatch calls the Handler method for this action.
	Dispatch(Handler) error
	// CountsAsUse reports whether running it ranks its result higher next
	// time. Copying a path or revealing a file does not.
	CountsAsUse() bool
	// Name is the action's type name in the Rust app's JSON.
	Name() string
}

// Handler does what each action asks. The engine implements it and checks
// every action again before it acts.
type Handler interface {
	Launch(Launch) error
	Open(Open) error
	Reveal(Reveal) error
	OpenURL(OpenURL) error
	Copy(Copy) error
	CopySecret(CopySecret) error
	CopyCalculation(CopyCalculation) error
	CopySavedCalculation(CopySavedCalculation) error
	DeleteCalculation(DeleteCalculation) error
	ClearCalculations(ClearCalculations) error
	CopyClip(CopyClip) error
	CopyClipText(CopyClipText) error
	DeleteClip(DeleteClip) error
	CopyPlainText(CopyPlainText) error
	ClearClipboard(ClearClipboard) error
	CopySnippet(CopySnippet) error
	OpenQuicklink(OpenQuicklink) error
	System(System) error
	Pin(Pin) error
	Unpin(Unpin) error
	Hide(Hide) error
	SetAlias(SetAlias) error
	SetHotkey(SetHotkey) error
	TurnOnCurrencyRates(TurnOnCurrencyRates) error
	Refresh(Refresh) error
	OpenSettings(OpenSettings) error
	CenterLauncher(CenterLauncher) error
	TypeQuery(TypeQuery) error
	Quit(Quit) error
	OpenUpNext(OpenUpNext) error
	AskCalendarAccess(AskCalendarAccess) error
}

// UpNextKind is what an Up Next item is.
type UpNextKind string

const (
	UpNextEvent    UpNextKind = "event"
	UpNextReminder UpNextKind = "reminder"
)

type (
	// Launch starts an app.
	Launch struct {
		Path string `json:"path"`
	}
	// Open opens a file or folder with its default app.
	Open struct {
		Path string `json:"path"`
	}
	// Reveal shows a file in the file manager.
	Reveal struct {
		Path string `json:"path"`
	}
	OpenURL struct {
		URL string `json:"url"`
	}
	Copy struct {
		Text string `json:"text"`
	}
	// CopySecret copies text that clipboard managers should not keep.
	CopySecret struct {
		Text string `json:"text"`
	}
	// CopyCalculation evaluates the query again, copies the result, and
	// saves it in the calculator history.
	CopyCalculation struct {
		Query string `json:"query"`
	}
	CopySavedCalculation struct {
		ID int64 `json:"id"`
	}
	DeleteCalculation struct {
		ID int64 `json:"id"`
	}
	ClearCalculations struct{}
	CopyClip          struct {
		ID int64 `json:"id"`
	}
	// CopyClipText copies a clipboard entry as plain text.
	CopyClipText struct {
		ID int64 `json:"id"`
	}
	DeleteClip struct {
		ID int64 `json:"id"`
	}
	// CopyPlainText removes the formatting of what the clipboard holds.
	CopyPlainText  struct{}
	ClearClipboard struct{}
	// CopySnippet copies a snippet with its placeholders filled; Query
	// fills {query}.
	CopySnippet struct {
		ID    int64  `json:"id"`
		Query string `json:"query"`
	}
	OpenQuicklink struct {
		ID    int64  `json:"id"`
		Query string `json:"query"`
	}
	System struct {
		Command SystemCommand `json:"command"`
	}
	Pin struct {
		ID ID `json:"id"`
	}
	Unpin struct {
		ID ID `json:"id"`
	}
	Hide struct {
		ID ID `json:"id"`
	}
	SetAlias struct {
		ID    ID     `json:"id"`
		Alias string `json:"alias"`
	}
	SetHotkey struct {
		ID       ID     `json:"id"`
		Shortcut string `json:"shortcut"`
	}
	TurnOnCurrencyRates struct{}
	Refresh             struct{}
	OpenSettings        struct{}
	CenterLauncher      struct{}
	// TypeQuery replaces the launcher's query, as the `?` list does.
	TypeQuery struct {
		Text string `json:"text"`
	}
	Quit struct{}
	// OpenUpNext opens the app that holds an Up Next item: Calendar for an
	// event, Reminders for a reminder.
	OpenUpNext struct {
		Kind UpNextKind `json:"kind"`
	}
	// AskCalendarAccess asks the OS to let Up Next read calendars and
	// reminders.
	AskCalendarAccess struct{}
)

func (a Launch) Dispatch(h Handler) error               { return h.Launch(a) }
func (a Open) Dispatch(h Handler) error                 { return h.Open(a) }
func (a Reveal) Dispatch(h Handler) error               { return h.Reveal(a) }
func (a OpenURL) Dispatch(h Handler) error              { return h.OpenURL(a) }
func (a Copy) Dispatch(h Handler) error                 { return h.Copy(a) }
func (a CopySecret) Dispatch(h Handler) error           { return h.CopySecret(a) }
func (a CopyCalculation) Dispatch(h Handler) error      { return h.CopyCalculation(a) }
func (a CopySavedCalculation) Dispatch(h Handler) error { return h.CopySavedCalculation(a) }
func (a DeleteCalculation) Dispatch(h Handler) error    { return h.DeleteCalculation(a) }
func (a ClearCalculations) Dispatch(h Handler) error    { return h.ClearCalculations(a) }
func (a CopyClip) Dispatch(h Handler) error             { return h.CopyClip(a) }
func (a CopyClipText) Dispatch(h Handler) error         { return h.CopyClipText(a) }
func (a DeleteClip) Dispatch(h Handler) error           { return h.DeleteClip(a) }
func (a CopyPlainText) Dispatch(h Handler) error        { return h.CopyPlainText(a) }
func (a ClearClipboard) Dispatch(h Handler) error       { return h.ClearClipboard(a) }
func (a CopySnippet) Dispatch(h Handler) error          { return h.CopySnippet(a) }
func (a OpenQuicklink) Dispatch(h Handler) error        { return h.OpenQuicklink(a) }
func (a System) Dispatch(h Handler) error               { return h.System(a) }
func (a Pin) Dispatch(h Handler) error                  { return h.Pin(a) }
func (a Unpin) Dispatch(h Handler) error                { return h.Unpin(a) }
func (a Hide) Dispatch(h Handler) error                 { return h.Hide(a) }
func (a SetAlias) Dispatch(h Handler) error             { return h.SetAlias(a) }
func (a SetHotkey) Dispatch(h Handler) error            { return h.SetHotkey(a) }
func (a TurnOnCurrencyRates) Dispatch(h Handler) error  { return h.TurnOnCurrencyRates(a) }
func (a Refresh) Dispatch(h Handler) error              { return h.Refresh(a) }
func (a OpenSettings) Dispatch(h Handler) error         { return h.OpenSettings(a) }
func (a CenterLauncher) Dispatch(h Handler) error       { return h.CenterLauncher(a) }
func (a TypeQuery) Dispatch(h Handler) error            { return h.TypeQuery(a) }
func (a Quit) Dispatch(h Handler) error                 { return h.Quit(a) }
func (a OpenUpNext) Dispatch(h Handler) error           { return h.OpenUpNext(a) }
func (a AskCalendarAccess) Dispatch(h Handler) error    { return h.AskCalendarAccess(a) }

// Running these ranks their result higher. The commands that act inside
// TinyDash (clear, settings, quit) count only for the System results that
// carry them; from the actions menu they come without a result ID.
func (Launch) CountsAsUse() bool               { return true }
func (Open) CountsAsUse() bool                 { return true }
func (Reveal) CountsAsUse() bool               { return false }
func (OpenURL) CountsAsUse() bool              { return true }
func (Copy) CountsAsUse() bool                 { return true }
func (CopySecret) CountsAsUse() bool           { return false }
func (CopyCalculation) CountsAsUse() bool      { return true }
func (CopySavedCalculation) CountsAsUse() bool { return true }
func (DeleteCalculation) CountsAsUse() bool    { return false }
func (ClearCalculations) CountsAsUse() bool    { return true }
func (CopyClip) CountsAsUse() bool             { return false }
func (CopyClipText) CountsAsUse() bool         { return false }
func (DeleteClip) CountsAsUse() bool           { return false }
func (CopyPlainText) CountsAsUse() bool        { return true }
func (ClearClipboard) CountsAsUse() bool       { return true }
func (CopySnippet) CountsAsUse() bool          { return true }
func (OpenQuicklink) CountsAsUse() bool        { return true }
func (System) CountsAsUse() bool               { return true }
func (Pin) CountsAsUse() bool                  { return false }
func (Unpin) CountsAsUse() bool                { return false }
func (Hide) CountsAsUse() bool                 { return false }
func (SetAlias) CountsAsUse() bool             { return false }
func (SetHotkey) CountsAsUse() bool            { return false }
func (TurnOnCurrencyRates) CountsAsUse() bool  { return false }
func (Refresh) CountsAsUse() bool              { return false }
func (OpenSettings) CountsAsUse() bool         { return true }
func (CenterLauncher) CountsAsUse() bool       { return false }
func (TypeQuery) CountsAsUse() bool            { return false }
func (Quit) CountsAsUse() bool                 { return true }
func (OpenUpNext) CountsAsUse() bool           { return false }
func (AskCalendarAccess) CountsAsUse() bool    { return false }

func (Launch) Name() string               { return "launch" }
func (Open) Name() string                 { return "open" }
func (Reveal) Name() string               { return "reveal" }
func (OpenURL) Name() string              { return "openUrl" }
func (Copy) Name() string                 { return "copy" }
func (CopySecret) Name() string           { return "copySecret" }
func (CopyCalculation) Name() string      { return "copyCalculation" }
func (CopySavedCalculation) Name() string { return "copySavedCalculation" }
func (DeleteCalculation) Name() string    { return "deleteCalculation" }
func (ClearCalculations) Name() string    { return "clearCalculations" }
func (CopyClip) Name() string             { return "copyClip" }
func (CopyClipText) Name() string         { return "copyClipText" }
func (DeleteClip) Name() string           { return "deleteClip" }
func (CopyPlainText) Name() string        { return "copyPlainText" }
func (ClearClipboard) Name() string       { return "clearClipboard" }
func (CopySnippet) Name() string          { return "copySnippet" }
func (OpenQuicklink) Name() string        { return "openQuicklink" }
func (System) Name() string               { return "system" }
func (Pin) Name() string                  { return "pin" }
func (Unpin) Name() string                { return "unpin" }
func (Hide) Name() string                 { return "hide" }
func (SetAlias) Name() string             { return "setAlias" }
func (SetHotkey) Name() string            { return "setHotkey" }
func (TurnOnCurrencyRates) Name() string  { return "turnOnCurrencyRates" }
func (Refresh) Name() string              { return "refresh" }
func (OpenSettings) Name() string         { return "openSettings" }
func (CenterLauncher) Name() string       { return "centerLauncher" }
func (TypeQuery) Name() string            { return "typeQuery" }
func (Quit) Name() string                 { return "quit" }
func (OpenUpNext) Name() string           { return "openUpNext" }
func (AskCalendarAccess) Name() string    { return "askCalendarAccess" }
