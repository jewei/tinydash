package core

// Result is one row of the launcher: what it shows and what it can do.
type Result struct {
	ID       ID
	Kind     Kind
	Title    string
	Subtitle string
	Icon     Icon
	// Actions[0] runs on Enter, Actions[1] on Mod+Enter; the actions menu
	// lists them all.
	Actions []ResultAction
	Pinned  bool
}

// ResultAction is an action with the label the actions menu shows.
type ResultAction struct {
	Label  string
	Action Action
	// Confirm, when set, is the question asked before the action runs.
	Confirm string
}

// Do is a ResultAction without a confirmation.
func Do(label string, action Action) ResultAction {
	return ResultAction{Label: label, Action: action}
}

// Scored is a result with its match score: higher ranks first.
type Scored struct {
	Score  uint32
	Result Result
}

// Kind says what a result is. The values are the names the Rust app used,
// so golden files compare as they are.
type Kind string

const (
	KindApp         Kind = "app"
	KindFile        Kind = "file"
	KindFolder      Kind = "folder"
	KindClipboard   Kind = "clipboard"
	KindSnippet     Kind = "snippet"
	KindQuicklink   Kind = "quicklink"
	KindEmoji       Kind = "emoji"
	KindCalculation Kind = "calculation"
	KindDateTime    Kind = "dateTime"
	KindPassword    Kind = "password"
	KindURL         Kind = "url"
	KindWebSearch   Kind = "webSearch"
	KindSystem      Kind = "system"
	KindColor       Kind = "color"
	KindKeyword     Kind = "keyword"
)

// Icon is exactly one of a file's icon, an emoji, a symbol, or a color swatch.
type Icon struct {
	File   string // path whose system icon shows
	Emoji  string // glyph
	Symbol Symbol
	Color  string // "#RRGGBB"
}

func FileIcon(path string) Icon   { return Icon{File: path} }
func EmojiIcon(glyph string) Icon { return Icon{Emoji: glyph} }
func SymbolIcon(name Symbol) Icon { return Icon{Symbol: name} }
func ColorIcon(hex string) Icon   { return Icon{Color: hex} }

// Symbol names a built-in icon.
type Symbol string

const (
	SymbolApp        Symbol = "app"
	SymbolFile       Symbol = "file"
	SymbolFolder     Symbol = "folder"
	SymbolText       Symbol = "text"
	SymbolImage      Symbol = "image"
	SymbolFiles      Symbol = "files"
	SymbolSnippet    Symbol = "snippet"
	SymbolLink       Symbol = "link"
	SymbolCalculator Symbol = "calculator"
	SymbolClock      Symbol = "clock"
	SymbolKey        Symbol = "key"
	SymbolGlobe      Symbol = "globe"
	SymbolLock       Symbol = "lock"
	SymbolMoon       Symbol = "moon"
	SymbolRestart    Symbol = "restart"
	SymbolPower      Symbol = "power"
	SymbolLogOut     Symbol = "logOut"
	SymbolTrash      Symbol = "trash"
	SymbolSettings   Symbol = "settings"
	SymbolQuit       Symbol = "quit"
	SymbolDisplay    Symbol = "display"
	SymbolContrast   Symbol = "contrast"
)
