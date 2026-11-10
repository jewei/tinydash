package settings

import (
	"strconv"
	"strings"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/engine"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/kit"
)

var categoryLabels = map[core.Category]string{
	core.CategoryAll: "All", core.CategoryApps: "Apps", core.CategoryFiles: "Files",
	core.CategoryClipboard: "Clipboard", core.CategorySnippets: "Snippets",
	core.CategoryEmoji: "Emoji", core.CategorySystem: "System",
}

type option struct{ value, label string }

var themes = []option{{"system", "Match System"}, {"light", "Light"}, {"dark", "Dark"}}

var engines = []option{{"google", "Google"}, {"duckDuckGo", "DuckDuckGo"}, {"bing", "Bing"}, {"brave", "Brave"}}

var skinTones = []option{
	{"0", "👋 Default"}, {"1", "👋🏻 Light"}, {"2", "👋🏼 Medium-light"},
	{"3", "👋🏽 Medium"}, {"4", "👋🏾 Medium-dark"}, {"5", "👋🏿 Dark"},
}

var languages = []option{{"zh", "Chinese (Simplified)"}, {"ms", "Malay"}, {"es", "Spanish"}}

// The limits match those that settings.Normalize and the clocks widget
// apply.
const (
	folderLimit = 50
	cityLimit   = 3
)

// View builds the window's content: give it to the window as
// ui.View(m.View).
func (m *Model) View(c *ui.Context) {
	v := m.values()
	p := kit.Themed(c, v.Theme)
	ui.Row(c).Fill().AlignItems(ui.Stretch).Background(p.Paper).Children(func() {
		m.sidebar(c, p)
		ui.Scroll(c).Grow(1).Padding(24, 32).Gap(12).Children(func() {
			ui.Text(c, string(m.section)).Font(kit.DisplayFont).FontSize(24).TextColor(p.Ink).Role(ui.RoleHeading)
			if m.err != "" {
				ui.Text(c, m.err).TextColor(p.Danger).Role(ui.RoleAlert)
			}
			switch m.section {
			case general:
				m.general(c, p, v)
			case widgets:
				m.widgets(c, p, v)
			case clipboard:
				m.clipboard(c, p, v)
			case search:
				m.search(c, p, v)
			case snippets:
				m.libraryView(c, p, m.editors[false])
			case quicklinks:
				m.libraryView(c, p, m.editors[true])
			}
		})
	})
	if m.confirmClear {
		m.clearDialog(c, p)
	}
}

func (m *Model) sidebar(c *ui.Context, p kit.Palette) {
	ui.Column(c).Width(180).Padding(16, 8).Gap(2).Background(p.Field).Role(ui.RoleNavigation).Label("Settings sections").Children(func() {
		for _, s := range sections {
			chosen := s == m.section
			b := ui.ButtonBase(c).Padding(6, 12).Radius(6).Justify(ui.Start).Label(string(s))
			ink, weight := p.Ink, 400
			switch {
			case chosen:
				b.Background(p.Selected).Current(ui.CurrentPage)
				ink, weight = p.SelectedInk, 600
			case b.Hovered():
				b.Background(p.Hover)
			}
			b.Children(func() { ui.Text(c, string(s)).TextColor(ink).FontWeight(weight) }).OnClick(func() { m.open(s) })
		}
	})
}

func (m *Model) general(c *ui.Context, p kit.Palette, v settings.Settings) {
	row(c, p, "Launcher shortcut", "Shows or hides TinyDash from any app.", func() { m.recorderView(c, p, v.Shortcut) })
	if err := m.rec.Err(); err != "" {
		ui.Text(c, err).TextColor(p.Danger).Role(ui.RoleAlert)
	}
	m.choice(c, p, "Appearance", "", themes, string(v.Theme), func(s *settings.Settings, value string) { s.Theme = settings.Theme(value) })
	m.toggle(c, p, "Hide when another app is focused", "", v.HideOnBlur, func(s *settings.Settings, on bool) { s.HideOnBlur = on })
	m.toggle(c, p, "Open at login", "Starts hidden, ready for the shortcut.", v.LaunchAtLogin, func(s *settings.Settings, on bool) { s.LaunchAtLogin = on })
	m.toggle(c, p, trayName, "", v.ShowTrayIcon, func(s *settings.Settings, on bool) { s.ShowTrayIcon = on })
	position := "Opens in the center of the screen."
	if v.LauncherPosition != nil {
		position = "Opens where you last dragged it."
	}
	row(c, p, "Launcher position", position, func() {
		// AccessDisabled, not Disabled: a disabled button would drop the
		// keyboard focus.
		ui.Button(c, "Center").AccessDisabled(v.LauncherPosition == nil).OnClick(func() {
			if m.values().LauncherPosition != nil {
				m.save(func(s *settings.Settings) { s.LauncherPosition = nil }, nil)
			}
		})
	})

	heading(c, p, "Tabs")
	intro(c, p, "Choose which tabs show after All, and in what order. Turn off “in All” to keep a tab's results out of All, such as emoji; its tab still finds them. A tab is never both hidden and out of All: turning one off turns the other on.")
	m.tabsView(c, p, v.Tabs)
}

func (m *Model) widgets(c *ui.Context, p kit.Palette, v settings.Settings) {
	intro(c, p, "Widgets show next to the results of an empty All search.")
	m.toggle(c, p, "Clocks", "Local time and up to three cities.", v.ShowClocks, func(s *settings.Settings, on bool) { s.ShowClocks = on })
	m.toggle(c, p, "Disk space", "Free space on the disk of your home folder.", v.ShowDiskSpace, func(s *settings.Settings, on bool) { s.ShowDiskSpace = on })
	heading(c, p, "Clock cities")
	m.listView(c, p, &m.cities, "City to add", "Tokyo, London, or Europe/Paris", v.ClockCities, cityLimit, true,
		func(s *settings.Settings) *[]string { return &s.ClockCities })
}

func (m *Model) clipboard(c *ui.Context, p kit.Palette, v settings.Settings) {
	m.toggle(c, p, "Save clipboard history", "Kept on this computer, unencrypted. Copies marked secret, such as passwords, are skipped.",
		v.ClipboardHistoryEnabled, func(s *settings.Settings, on bool) { s.ClipboardHistoryEnabled = on })
	m.number(c, p, &m.limit, "Entries to keep", "Pinned entries do not count.", int(v.ClipboardHistoryLimit), 1, settings.ClipboardLimitMax,
		func(s *settings.Settings, n int) { s.ClipboardHistoryLimit = uint32(n) })
	if m.richClipboard {
		m.toggle(c, p, "Save images", "Up to 32 images, not counting pins. Large images are skipped.",
			v.ClipboardCaptureImages, func(s *settings.Settings, on bool) { s.ClipboardCaptureImages = on })
		m.toggle(c, p, "Save copied files", "Saves references to files, not their contents.",
			v.ClipboardCaptureFiles, func(s *settings.Settings, on bool) { s.ClipboardCaptureFiles = on })
	}
	row(c, p, "Clear history", "Deletes every entry except pinned ones.", func() {
		ui.Button(c, "Clear History…").Bind(&m.clearButton).OnClick(func() { m.confirmClear = true })
	})
}

func (m *Model) search(c *ui.Context, p kit.Palette, v settings.Settings) {
	m.choice(c, p, "Web search", "Used for the web search at the end of All results.", engines, string(v.SearchEngine),
		func(s *settings.Settings, value string) { s.SearchEngine = settings.SearchEngine(value) })
	m.toggle(c, p, "Currency rates", "Off until you turn it on. Downloads the daily ECB rate table; your amounts stay on this computer. Turning it off deletes the saved rates.",
		v.CurrencyRatesEnabled, func(s *settings.Settings, on bool) { s.CurrencyRatesEnabled = on })
	m.choice(c, p, "Emoji skin tone", "", skinTones, strconv.Itoa(int(v.EmojiSkinTone)), func(s *settings.Settings, value string) {
		if tone, ok := skinTone(value); ok {
			s.EmojiSkinTone = tone
		}
	})

	heading(c, p, "Emoji keywords")
	intro(c, p, "English names and :shortcodes: always work. Add keywords in:")
	for _, language := range languages {
		code := settings.EmojiLanguage(language.value)
		on := false
		for _, l := range v.EmojiLanguages {
			on = on || l == code
		}
		m.toggle(c, p, language.label, "", on, func(s *settings.Settings, on bool) {
			var list []settings.EmojiLanguage
			for _, l := range s.EmojiLanguages {
				if l != code {
					list = append(list, l)
				}
			}
			if on {
				list = append(list, code)
			}
			s.EmojiLanguages = list
		})
	}

	heading(c, p, "Folders")
	intro(c, p, "TinyDash searches the names of files and folders here. It never reads file contents. Hidden files are skipped.")
	m.listView(c, p, &m.folders, "Folder to add", "~/Projects", v.FileSearchFolders, folderLimit, false,
		func(s *settings.Settings) *[]string { return &s.FileSearchFolders })
	heading(c, p, "Skip folders named")
	m.listView(c, p, &m.skipped, "Folder name to skip", "node_modules", v.FileSearchExcludedDirs, folderLimit, false,
		func(s *settings.Settings) *[]string { return &s.FileSearchExcludedDirs })
	if m.mac {
		m.toggle(c, p, "Also search with Spotlight", "The Files tab also finds names anywhere in your home folder, from the Spotlight index, below the results from these folders. Hidden files and the folder names above are skipped.",
			v.SpotlightFiles, func(s *settings.Settings, on bool) { s.SpotlightFiles = on })
	}

	heading(c, p, "Aliases")
	intro(c, p, "Typing an alias puts its result first. Up to 500.")
	m.personalView(c, p, &m.aliases, personalText{
		list:  "Aliases",
		empty: "None. To add one, choose Add Alias in a result's actions (" + modKeys[m.mac] + " K).",
		value: func(r engine.Listed) { ui.Text(c, r.Value).FontWeight(600).TextColor(p.Ink) },
		label: func(r engine.Listed) string { return "Remove the alias " + r.Value + " of " + r.Title },
	}, m.backend.RemoveAlias, m.backend.Aliases)

	heading(c, p, "Hotkeys")
	intro(c, p, "Keys that run a result from any app. Up to 50.")
	m.personalView(c, p, &m.hotkeys, personalText{
		list:  "Hotkeys",
		empty: "None. To add one, choose Set Hotkey in a result's actions (" + modKeys[m.mac] + " K).",
		value: func(r engine.Listed) { keyCaps(c, p, kit.DisplayKeys(r.Value, m.mac, kit.SuperName)...) },
		label: func(r engine.Listed) string { return "Remove the hotkey of " + r.Title },
	}, m.backend.RemoveHotkey, m.backend.Hotkeys)

	heading(c, p, "Hidden results")
	intro(c, p, "Search leaves these out. Up to 500.")
	m.personalView(c, p, &m.hidden, personalText{
		list:   "Hidden results",
		empty:  "None. To hide one, choose Hide from Results in its actions (" + modKeys[m.mac] + " K).",
		button: "Show Again",
		label:  func(r engine.Listed) string { return "Show " + r.Title + " again" },
	}, m.backend.Unhide, m.backend.Hidden)
}

// row is one labeled setting: text on the left, its control on the right.
// The row is a group named and described by its text, so a screen reader
// reads the description with the control.
func row(c *ui.Context, p kit.Palette, label, description string, control func()) {
	g := ui.Row(c).Gap(16).Padding(8, 0).AlignItems(ui.Center).BorderWidth(0, 0, 1, 0).BorderColor(p.Rule).Role(ui.RoleGroup).Label(label)
	if description != "" {
		g.Description(description)
	}
	g.Children(func() {
		ui.Column(c).Grow(1).Shrink(1).Gap(2).Children(func() {
			ui.Text(c, label).TextColor(p.Ink).AccessHidden(true)
			if description != "" {
				ui.Text(c, description).FontSize(12).TextColor(p.Muted).AccessHidden(true)
			}
		})
		control()
	})
}

func heading(c *ui.Context, p kit.Palette, text string) {
	ui.Text(c, text).FontSize(17).FontWeight(600).TextColor(p.Ink).Margin(12, 0, 0, 0).Role(ui.RoleHeading)
}

func intro(c *ui.Context, p kit.Palette, text string) {
	ui.Text(c, text).TextColor(p.Muted)
}

func (m *Model) toggle(c *ui.Context, p kit.Palette, label, description string, value bool, set func(*settings.Settings, bool)) {
	row(c, p, label, description, func() {
		on := value
		if ui.Switch(c, &on).Label(label).Changed() {
			m.save(func(s *settings.Settings) { set(s, on) }, nil)
		}
	})
}

// skinTone reads a skin tone option. A value that does not fit a uint8 is
// not a tone, and must not wrap to one.
func skinTone(value string) (uint8, bool) {
	n, err := strconv.ParseUint(value, 10, 8)
	return uint8(n), err == nil
}

func (m *Model) choice(c *ui.Context, p kit.Palette, label, description string, options []option, value string, set func(*settings.Settings, string)) {
	row(c, p, label, description, func() {
		labels := make([]string, len(options))
		chosen := ""
		for i, o := range options {
			labels[i] = o.label
			if o.value == value {
				chosen = o.label
			}
		}
		if !ui.Select(c, &chosen, labels).Label(label).Width(200).Changed() {
			return
		}
		for _, o := range options {
			if o.label == chosen {
				m.save(func(s *settings.Settings) { set(s, o.value) }, nil)
			}
		}
	})
}

func (m *Model) number(c *ui.Context, p kit.Palette, f *numberField, label, description string, value, lo, hi int, set func(*settings.Settings, int)) {
	if !f.editing {
		f.draft = strconv.Itoa(value)
	}
	row(c, p, label, description, func() {
		in := ui.TextInput(c, &f.draft).Label(label).Width(96)
		in.OnTextKey(func(k ui.TextKey) bool {
			if k.Key != ui.KeyEnter || k.Mods != 0 {
				return false
			}
			m.commit(f, value, lo, hi, set)
			return true
		})
		if in.Changed() {
			f.editing = true
		}
		if f.editing && !in.Focused() {
			m.commit(f, value, lo, hi, set)
		}
	})
}

// recorderView is the launcher shortcut: a click, Enter, or Space records
// the next keys pressed, and Escape, another click, or leaving it stops.
func (m *Model) recorderView(c *ui.Context, p kit.Palette, shortcut string) {
	const label = "Launcher shortcut"
	keys := kit.DisplayKeys(shortcut, m.mac, kit.SuperName)
	name := label + ": " + strings.Join(keys, " ")
	edge := p.Border
	if m.rec.On() {
		name = label + ": press the new keys, or Escape to cancel"
		edge = p.Accent
	}
	rec := ui.Row(c).Bind(&m.recorder).Focusable().MinWidth(160).Height(32).Padding(0, 12).Radius(6).
		Border(1, edge).Background(p.Field).Justify(ui.Center).AlignItems(ui.Center).
		Role(ui.RoleButton).Label(name)
	rec.HandleInput(func(ev ui.InputEvent) bool {
		if ev.Kind != ui.InputKeyDown {
			return false
		}
		switch {
		case m.rec.On() && ev.Key == ui.KeyEscape:
			m.stopRecording("")
		case m.rec.On():
			if keys := kit.Accelerator(ev, m.mac); keys != "" {
				m.stopRecording(keys)
			}
		case ev.Key == ui.KeyEnter || ev.Key == ui.KeySpace:
			if !ev.Repeat {
				m.startRecording()
			}
		default:
			return false
		}
		return true
	})
	rec.OnClick(func() {
		if m.rec.On() {
			m.stopRecording("")
		} else {
			m.startRecording()
		}
	})
	focused := rec.Focused()
	if m.rec.On() && m.recorderFocused && !focused {
		m.stopRecording("")
	}
	m.recorderFocused = focused
	rec.Children(func() {
		if m.rec.On() {
			ui.Text(c, "Press keys…").TextColor(p.Ink)
			return
		}
		keyCaps(c, p, keys...)
	})
}

// keyCaps shows the keys of a shortcut, such as ⌃ ⇧ Space.
func keyCaps(c *ui.Context, p kit.Palette, keys ...string) {
	ui.Row(c).Gap(3).Children(func() {
		for _, k := range keys {
			ui.Text(c, k).FontSize(12).Padding(0, 5).Radius(4).Border(1, p.Border).TextColor(p.Muted)
		}
	})
}

// listView shows a list of text with Remove on each entry, and a field and
// Add below it. A failure shows next to the field: the page's error line
// may be scrolled out of view.
func (m *Model) listView(c *ui.Context, p kit.Palette, l *listEditor, label, placeholder string, items []string, limit int, ignoreCase bool, get func(*settings.Settings) *[]string) {
	full := len(items) >= limit
	add := func() { m.addTo(l, items, limit, ignoreCase, get) }
	ui.Column(c).Gap(6).Children(func() {
		ui.Column(c).Role(ui.RoleList).Label(label).Children(func() {
			if len(items) == 0 {
				ui.Text(c, "None").TextColor(p.Muted)
			}
			for _, item := range items {
				ui.Row(c).Key(item).Gap(8).Padding(4, 0).AlignItems(ui.Center).Role(ui.RoleListItem).Children(func() {
					ui.Text(c, item).TextColor(p.Ink).Grow(1)
					linkButton(c, p, "Remove", "Remove "+item).OnClick(func() { m.removeFrom(l, item, get) })
				})
			}
		})
		ui.Row(c).Gap(8).Children(func() {
			in := ui.TextInput(c, &l.draft).Bind(&l.field).Label(label).Placeholder(placeholder).Grow(1)
			in.OnTextKey(func(k ui.TextKey) bool {
				if k.Key != ui.KeyEnter || k.Mods != 0 {
					return false
				}
				add()
				return true
			})
			if in.Changed() {
				l.notice = ""
			}
			ui.Button(c, "Add").Label("Add to " + label).Disabled(full).OnClick(add)
		})
		status := l.notice
		if status == "" && full {
			status = "The list is full (" + strconv.Itoa(limit) + "). Remove one to add another."
		}
		if status != "" {
			ui.Text(c, status).FontSize(12).TextColor(p.Muted).Role(ui.RoleStatus)
		}
	})
}

func linkButton(c *ui.Context, p kit.Palette, text, label string) ui.Element {
	b := ui.ButtonBase(c).Padding(2, 6).Radius(4).Label(label)
	if b.Hovered() {
		b.Background(p.Hover)
	}
	return b.Children(func() { ui.Text(c, text).TextColor(p.Accent) })
}

// tabsView lists the launcher's tabs after All: show or hide each one, keep
// its results in or out of All, and move it. Rows are keyed by category,
// so a button keeps the focus as its row moves; at the top or bottom, the
// focus goes to the other move button.
func (m *Model) tabsView(c *ui.Context, p kit.Palette, tabs []settings.Tab) {
	moved := m.moved
	m.moved = nil
	ui.Column(c).Role(ui.RoleList).Label("Tabs, in order").Children(func() {
		ui.Row(c).Gap(8).Padding(6, 0).Role(ui.RoleListItem).Children(func() {
			ui.Text(c, categoryLabels[core.CategoryAll]).TextColor(p.Ink).Grow(1)
			ui.Text(c, "Always shown, always first").TextColor(p.Muted)
		})
		for i, tab := range tabs {
			name := categoryLabels[tab.Category]
			ui.Row(c).Key(tab.Category).Gap(8).Padding(6, 0).AlignItems(ui.Center).Role(ui.RoleListItem).Children(func() {
				ui.Text(c, name).TextColor(p.Ink).Grow(1)
				first, last := i == 0, i == len(tabs)-1
				up := linkButton(c, p, "↑", "Move "+name+" up").Disabled(first).OnClick(func() { m.moveTab(tab.Category, true) })
				down := linkButton(c, p, "↓", "Move "+name+" down").Disabled(last).OnClick(func() { m.moveTab(tab.Category, false) })
				if moved != nil && moved.category == tab.Category {
					if moved.up && !first || last {
						up.Focus()
					} else {
						down.Focus()
					}
				}
				shown, inAll := tab.Shown, tab.InAll
				if ui.Switch(c, &shown).Label("Show " + name).Changed() {
					m.setTab(tab.Category, true, shown)
				}
				ui.Text(c, "Show").FontSize(12).TextColor(p.Muted)
				if ui.Switch(c, &inAll).Label(name + " in All").Changed() {
					m.setTab(tab.Category, false, inAll)
				}
				ui.Text(c, "In All").FontSize(12).TextColor(p.Muted)
			})
		}
	})
}

// personalText is what a personal list says.
type personalText struct {
	list, empty string
	// button is the text of each row's button; "" is Remove.
	button string
	// value, if set, draws the alias or keys before the title.
	value func(engine.Listed)
	label func(engine.Listed) string
}

// personalView lists the results of l, each with a button that undoes the
// change. After one is undone, the row now in its place, or the list when
// it is empty, takes the focus, so keyboard users keep their place.
func (m *Model) personalView(c *ui.Context, p kit.Palette, l *personalList, text personalText, undo func(core.ID, func(error)), reload func() []engine.Listed) {
	focus := -1
	if l.refocus {
		focus = l.focus
	}
	l.refocus = false
	button := text.button
	if button == "" {
		button = "Remove"
	}
	list := ui.Column(c).Role(ui.RoleList).Label(text.list)
	list.Children(func() {
		if len(l.items) == 0 {
			ui.Text(c, text.empty).TextColor(p.Muted)
		}
		for i, r := range l.items {
			ui.Row(c).Key(r.ID).Gap(8).Padding(4, 0).AlignItems(ui.Center).Role(ui.RoleListItem).Children(func() {
				ui.Column(c).Grow(1).Children(func() {
					ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
						if text.value != nil {
							text.value(r)
						}
						ui.Text(c, r.Title).TextColor(p.Ink)
					})
					ui.Text(c, r.Subtitle).FontSize(12).TextColor(p.Muted)
				})
				b := linkButton(c, p, button, text.label(r)).OnClick(func() { l.remove(r.ID, i, undo, reload) })
				if focus >= 0 && i == min(focus, len(l.items)-1) {
					b.Focus()
				}
			})
		}
	})
	if focus >= 0 && len(l.items) == 0 {
		list.Focusable().Focus()
	}
	if l.err != "" {
		ui.Text(c, l.err).FontSize(12).TextColor(p.Danger).Role(ui.RoleStatus)
	}
}

func (m *Model) clearDialog(c *ui.Context, p kit.Palette) {
	ui.Overlay(c, func() {
		back := ui.Column(c).Absolute().Left(0).Top(0).Right(0).Bottom(0).
			Background(p.Backdrop).Justify(ui.Center).AlignItems(ui.Center).Modal()
		back.Children(func() {
			panel := ui.Column(c).Width(360).Padding(24).Gap(16).Radius(10).Border(1, p.Border).
				Background(p.Paper).Shadow(0, 12, 40, 0, p.Shadow).Role(ui.RoleAlertDialog).Label("Confirm")
			panel.Children(func() {
				ui.Text(c, "Delete all clipboard history except pinned entries?").TextColor(p.Ink)
				ui.Row(c).Gap(8).Justify(ui.End).Children(func() {
					// Cancel comes first, so it has the focus and Enter is safe.
					ui.Button(c, "Cancel").AutoFocus().OnClick(m.closeClear)
					ui.Button(c, "Clear History").Background(p.Danger).TextColor(p.DangerInk).OnClick(m.clearHistory)
				})
			})
			if panel.PressedOutside() {
				m.closeClear()
			}
		})
		if back.OverlayShortcut(0, ui.KeyEscape) {
			m.closeClear()
		}
	})
}

var libraryHelp = map[bool]string{
	false: "Enter copies the text. Placeholders: {date}, {time}, {datetime}, {clipboard}, and {query}, the text typed after the keyword, as in “hi Sam” (it needs a keyword).",
	true:  "Opens an http, https, or mailto URL, or an absolute or ~ path. {query} is replaced by text typed after the keyword, as in “jira ABC-12”. Also: {clipboard}, {date}, {time}, and {datetime}.",
}

func (m *Model) libraryView(c *ui.Context, p kit.Palette, ed *libraryEditor) {
	focus := ed.focus
	ed.focus = focusNone
	ui.Row(c).Gap(24).AlignItems(ui.Start).Children(func() {
		ui.Column(c).Width(220).Gap(8).Children(func() {
			ui.Button(c, "New "+ed.noun()).OnClick(func() { ed.edit(ed.blank()) })
			items := ed.items()
			ui.Column(c).Gap(2).Role(ui.RoleList).Label(ed.noun() + "s").Children(func() {
				if len(items) == 0 {
					ui.Text(c, "No "+strings.ToLower(ed.noun())+"s yet").TextColor(p.Muted)
				}
				for _, it := range items {
					name := it.Name
					if it.Keyword != "" {
						name += " " + it.Keyword
					}
					b := ui.ButtonBase(c).Key(it.ID).Padding(6, 8).Radius(6).Justify(ui.Start).Label(name)
					if it.ID == ed.draft.ID {
						b.Background(p.Selected).Current(ui.CurrentPage)
					} else if b.Hovered() {
						b.Background(p.Hover)
					}
					b.Children(func() {
						ui.Row(c).Gap(8).Children(func() {
							ui.Text(c, it.Name).TextColor(p.Ink).Grow(1).SingleLine()
							if it.Keyword != "" {
								ui.Text(c, it.Keyword).FontSize(12).TextColor(p.Muted)
							}
						})
					}).OnClick(func() { ed.edit(it) })
				}
			})
		})
		ui.Column(c).Grow(1).Gap(8).Children(func() { m.libraryForm(c, p, ed, focus) })
	})
}

// libraryForm edits the open item. It sets no length limits: the backend
// counts characters, as the user does, and says what is too long.
func (m *Model) libraryForm(c *ui.Context, p kit.Palette, ed *libraryEditor, focus libraryFocus) {
	ui.Text(c, "Name").TextColor(p.Ink).AccessHidden(true)
	name := ui.TextInput(c, &ed.draft.Name).Label("Name")
	if focus == focusName {
		name.Focus()
	}
	ui.Text(c, "Keyword (optional, one word)").TextColor(p.Ink).AccessHidden(true)
	ui.TextInput(c, &ed.draft.Keyword).Label("Keyword")
	if ed.quicklink {
		ui.Text(c, "URL or path").TextColor(p.Ink).AccessHidden(true)
		ui.TextInput(c, &ed.draft.Text).Label("URL or path").Placeholder("https://example.com/search?q={query}")
	} else {
		ui.Text(c, "Text").TextColor(p.Ink).AccessHidden(true)
		ui.TextArea(c, &ed.draft.Text).Label("Text").Height(160)
	}
	ui.Text(c, libraryHelp[ed.quicklink]).FontSize(12).TextColor(p.Muted)
	if ed.err != "" {
		ui.Text(c, ed.err).TextColor(p.Danger).Role(ui.RoleAlert)
	}
	ui.Row(c).Gap(8).Justify(ui.End).AlignItems(ui.Center).Children(func() {
		if ed.draft.ID != 0 {
			if ed.confirming {
				question := ui.Text(c, "Delete “"+ed.savedName()+"”?").TextColor(p.Ink)
				keep := ui.Button(c, "Keep").DescribedBy(question).OnClick(ed.keep)
				if focus == focusKeep {
					keep.Focus()
				}
				ui.Button(c, "Delete").Label("Delete").Background(p.Danger).TextColor(p.DangerInk).OnClick(ed.remove)
			} else {
				del := ui.Button(c, "Delete").OnClick(ed.askDelete)
				if focus == focusDelete {
					del.Focus()
				}
			}
		}
		ui.PrimaryButton(c, "Save "+ed.noun()).Disabled(ed.busy).OnClick(ed.save)
	})
}
