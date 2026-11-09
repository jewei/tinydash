package launcher

import (
	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
)

// glyphs are the Solid app's icons (src/ui/Icon.tsx): a 24-unit grid with
// round 1.75-unit strokes, drawn in the text color.
var glyphs = func() map[string]*ui.SVG {
	shapes := map[string]string{
		string(core.SymbolApp):        `<rect x="4" y="4" width="7" height="7" rx="2"/><rect x="13" y="4" width="7" height="7" rx="2"/><rect x="4" y="13" width="7" height="7" rx="2"/><rect x="13" y="13" width="7" height="7" rx="2"/>`,
		string(core.SymbolFile):       `<path d="M7 3h7l5 5v13H7zM14 3v5h5"/>`,
		string(core.SymbolFolder):     `<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>`,
		string(core.SymbolText):       `<path d="M5 6h14M5 10h14M5 14h10M5 18h7"/>`,
		string(core.SymbolImage):      `<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="9" cy="10" r="1.5"/><path d="M21 16l-5-5-8 8"/>`,
		string(core.SymbolFiles):      `<rect x="8" y="3" width="12" height="15" rx="2"/><path d="M5 7v12a2 2 0 0 0 2 2h9"/>`,
		string(core.SymbolSnippet):    `<path d="M8 4C6 4 5 5 5 7v2c0 1.5-1 3-2 3 1 0 2 1.5 2 3v2c0 2 1 3 3 3M16 4c2 0 3 1 3 3v2c0 1.5 1 3 2 3-1 0-2 1.5-2 3v2c0 2-1 3-3 3"/>`,
		string(core.SymbolLink):       `<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>`,
		string(core.SymbolCalculator): `<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M8 7h8M8.5 12h.01M12 12h.01M15.5 12h.01M8.5 16h.01M12 16h.01M15.5 16h.01"/>`,
		string(core.SymbolClock):      `<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>`,
		string(core.SymbolKey):        `<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M17 6l3 3M15 8l2 2"/>`,
		string(core.SymbolGlobe):      `<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.5 3.5 5.5 3.5 9s-1 6.5-3.5 9c-2.5-2.5-3.5-5.5-3.5-9s1-6.5 3.5-9z"/>`,
		string(core.SymbolLock):       `<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>`,
		string(core.SymbolMoon):       `<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>`,
		string(core.SymbolDisplay):    `<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>`,
		string(core.SymbolContrast):   `<circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor"/>`,
		string(core.SymbolRestart):    `<path d="M4 12a8 8 0 1 0 2.3-5.7L4 8.5M4 4v4.5h4.5"/>`,
		string(core.SymbolPower):      `<path d="M12 3v9M6.3 6.3a8 8 0 1 0 11.4 0"/>`,
		string(core.SymbolLogOut):     `<path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 16l-4-4 4-4M6 12h10"/>`,
		string(core.SymbolTrash):      `<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>`,
		string(core.SymbolSettings):   `<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>`,
		string(core.SymbolQuit):       `<circle cx="12" cy="12" r="9"/><path d="M9 9l6 6M15 9l-6 6"/>`,
		"search":                      `<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>`,
		"pin":                         `<path d="M12 16v5M8 3h8l-1 6 3 3v2H6v-2l3-3z"/>`,
		"warning":                     `<path d="M12 4l9 16H3zM12 10v4M12 17h.01"/>`,
		"disk":                        `<rect x="3" y="12" width="18" height="8" rx="2"/><path d="M5 12l2.5-7h9L19 12M17 16h.01"/>`,
	}
	out := make(map[string]*ui.SVG, len(shapes))
	for name, shape := range shapes {
		out[name] = ui.MustParseSVG([]byte(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">` + shape + `</svg>`))
	}
	return out
}()

// kinds are each result kind's label, shown on the selected row, and the
// glyph of a result that has no icon of its own.
var kinds = map[core.Kind]struct{ label, glyph string }{
	core.KindApp:         {"Application", string(core.SymbolApp)},
	core.KindFile:        {"File", string(core.SymbolFile)},
	core.KindFolder:      {"Folder", string(core.SymbolFolder)},
	core.KindClipboard:   {"Clipboard", string(core.SymbolText)},
	core.KindSnippet:     {"Snippet", string(core.SymbolSnippet)},
	core.KindQuicklink:   {"Quicklink", string(core.SymbolLink)},
	core.KindEmoji:       {"Emoji", string(core.SymbolText)},
	core.KindCalculation: {"Calculation", string(core.SymbolCalculator)},
	core.KindDateTime:    {"Date & Time", string(core.SymbolClock)},
	core.KindPassword:    {"Password", string(core.SymbolKey)},
	core.KindURL:         {"Clean URL", string(core.SymbolLink)},
	core.KindWebSearch:   {"Web Search", string(core.SymbolGlobe)},
	core.KindSystem:      {"Command", string(core.SymbolSettings)},
	core.KindColor:       {"Color", string(core.SymbolImage)},
	core.KindKeyword:     {"Keyword", "search"},
}

var categoryLabels = map[core.Category]string{
	core.CategoryAll: "All", core.CategoryApps: "Apps", core.CategoryFiles: "Files",
	core.CategoryClipboard: "Clipboard", core.CategorySnippets: "Snippets",
	core.CategoryEmoji: "Emoji", core.CategorySystem: "System",
}
