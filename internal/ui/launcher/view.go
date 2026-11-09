package launcher

import (
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// View builds the panel's content: give it to the window as ui.View(m.View).
func (m *Model) View(c *ui.Context) {
	p := kit.Themed(c, m.backend.Settings().Theme)
	// The menu and the dialogs take the keyboard while they show; keys
	// behind them must not act.
	if !m.overlay() {
		for _, b := range keymap {
			if b.when == nil || b.when(m) {
				c.OnShortcut(b.mods, b.key, func() { b.run(m, c.ShortcutRepeated()) })
			}
		}
	}
	var actions ui.Element
	ui.Column(c).Fill().ObserveInput(m.observe).Children(func() {
		m.searchBar(c, p)
		m.tabBar(c, p)
		switch {
		case m.notice != "":
			noticeBar(c, p, m.notice, nil)
		case m.warning() != "":
			noticeBar(c, p, m.warning(), func() {
				m.dismissWarning()
				m.focusInput()
			})
		}
		m.body(c, p)
		actions = m.footer(c, p)
	})
	switch {
	case m.confirm != nil:
		m.confirmView(c, p)
	case m.alias != nil:
		m.aliasView(c, p)
	case m.hotkey != nil:
		m.hotkeyView(c, p)
	case m.menu != nil:
		m.menuView(c, p, actions)
	}
}

// observe notes whether the last key came as a repeat; a press of the
// pointer is no repeat.
func (m *Model) observe(ev ui.InputEvent) {
	switch ev.Kind {
	case ui.InputKeyDown:
		m.repeat = ev.Repeat
	case ui.InputPointerDown:
		m.repeat = false
	case ui.InputPointerUp:
		// A press on a bar that is let go of moves the panel no more.
		if m.dragPending {
			m.dragPending = false
			m.backend.DragEnded()
		}
	}
}

func (m *Model) searchBar(c *ui.Context, p kit.Palette) {
	ui.Row(c).Padding(16, 24, 8, 24).Gap(12).AlignItems(ui.Center).Children(func() {
		ui.Icon(c, glyphs["search"]).FontSize(20).TextColor(p.Accent)
		field := ui.TextInputBase(c, &m.query).Bind(&m.input).AutoFocus().Grow(1).FontSize(22).
			Placeholder("Search apps, files, clipboard, and more").Label("Search")
		// The field takes Enter and its editing keys, such as Mod+Backspace,
		// before any shortcut; it leaves those the keymap does not take.
		field.OnTextKey(func(k ui.TextKey) bool { return m.press(k.Mods, k.Key, m.repeat) })
		if field.Changed() {
			m.edited()
		}
		// A query put in the field, as Up brings one back, is typed on at
		// its end.
		if m.caretToEnd {
			n := utf8.RuneCountInString(m.query)
			field.SetTextSelection(n, n)
			m.caretToEnd = false
		}
	})
}

func (m *Model) tabBar(c *ui.Context, p kit.Palette) {
	tabs := m.tabs()
	m.dragArea(ui.Row(c).Padding(4, 16, 12, 16).Gap(4).AlignItems(ui.Center).BorderWidth(0, 0, 1, 0).BorderColor(p.Rule)).Children(func() {
		ui.Row(c).Gap(4).Role(ui.RoleTabList).Label("Categories").Grow(1).Children(func() {
			for _, tab := range tabs {
				chosen := tab == m.category
				t := ownPress(ui.ButtonBase(c)).Padding(4, 12).Radius(999).Role(ui.RoleTab).Label(categoryLabels[tab]).AccessSelected(chosen)
				ink, weight := p.Muted, 400
				switch {
				case chosen:
					t.Background(p.Selected)
					ink, weight = p.SelectedInk, 600
				case t.Hovered():
					t.Background(p.Hover)
				}
				t.Children(func() { ui.Text(c, categoryLabels[tab]).TextColor(ink).FontWeight(weight) })
				t.OnClick(func() {
					m.setCategory(tab)
					m.input.Focus()
				})
			}
		})
		if len(tabs) > 1 {
			ui.Row(c).Gap(4).AlignItems(ui.Center).Children(func() {
				keyCaps(c, p, "Tab")
				ui.Text(c, "next").FontSize(12).TextColor(p.Muted)
			})
		}
	})
}

// dragArea makes the empty parts of a bar move the panel, which has no
// title bar; its buttons still take their clicks. The focus stays in the
// search field, so typing goes on after the move.
func (m *Model) dragArea(bar ui.Element) ui.Element {
	return bar.DragWindow().KeepFocus().HandleInput(func(ev ui.InputEvent) bool {
		if ev.Kind == ui.InputPointerDown && ev.Button == 0 {
			m.dragPending = true
			m.backend.DragStarted()
		}
		return false // the press still moves the window
	})
}

// ownPress keeps a press on a control inside a drag area from reaching
// the area's handler: mygo gives a press to the innermost element with an
// input function, and the press is the control's click, not a drag.
func ownPress(control ui.Element) ui.Element {
	return control.HandleInput(func(ui.InputEvent) bool { return false })
}

// noticeBar says what went wrong under the tabs. A startup warning has a
// Dismiss button; an action's failure goes with the next edit.
func noticeBar(c *ui.Context, p kit.Palette, text string, dismiss func()) {
	ui.Row(c).Padding(8, 24).Gap(8).AlignItems(ui.Center).Background(p.Notice).Role(ui.RoleAlert).Children(func() {
		ui.Icon(c, glyphs["warning"]).FontSize(16)
		ui.Text(c, text).FontSize(12).Grow(1)
		if dismiss != nil {
			footerButton(c, p, "Dismiss", []string{"esc"}).OnClick(dismiss)
		}
	})
}

// body shows the results, and beside them the widget pane on an empty All
// search, or else the selected result's details.
func (m *Model) body(c *ui.Context, p kit.Palette) {
	m.loadDetails()
	widgets, details := m.showsWidgets(), m.showsDetails()
	if !widgets && !details {
		m.resultsView(c, p)
		return
	}
	// The Solid app split the body 11 to 9 between the results and the pane.
	ui.Row(c).Grow(1).AlignItems(ui.Stretch).Children(func() {
		ui.Column(c).Grow(11).Children(func() { m.resultsView(c, p) })
		if widgets {
			m.widgetPane(c, p)
		} else {
			m.detailsPane(c, p)
		}
	})
}

func (m *Model) resultsView(c *ui.Context, p kit.Palette) {
	rows := m.rows()
	switch {
	case m.clipboardOff():
		empty(c, p, "Clipboard history is off", func() {
			muted(c, p, "TinyDash saves text you copy, on this computer only. Passwords are skipped.")
			ui.PrimaryButton(c, "Turn On Clipboard History").OnClick(m.enableClipboard)
			if m.enableError != "" {
				ui.Text(c, m.enableError).TextColor(p.Danger).Role(ui.RoleAlert)
			}
		})
	case len(rows) == 0:
		m.emptyResults(c, p)
	default:
		m.list.Key = func(i int) any { return rows[i].ID }
		m.list.Label = func(i int) string { return rows[i].Title }
		ui.List(c, &m.list, len(rows), func(i int) { m.row(c, p, rows[i], i) }).Grow(1).Padding(8).Label("Results")
	}
}

// emptyResults says what an empty list means, by tab, before and after
// typing.
func (m *Model) emptyResults(c *ui.Context, p kit.Palette) {
	q := strings.TrimSpace(m.query)
	switch {
	case q != "":
		empty(c, p, "No results", func() { muted(c, p, "Nothing matches “"+q+"”.") })
	case m.category == core.CategoryClipboard:
		empty(c, p, "Nothing copied yet", func() { muted(c, p, "Text you copy appears here, newest first.") })
	case m.category == core.CategorySnippets:
		empty(c, p, "No snippets yet", func() {
			muted(c, p, "Save text you type often, or links you open with a keyword.")
			ui.PrimaryButton(c, "Add in Settings").OnClick(m.openSettings)
		})
	case m.category == core.CategoryFiles:
		empty(c, p, "Search your files", func() {
			muted(c, p, "Type part of a file or folder name. Choose the folders in Settings.")
		})
	default:
		empty(c, p, "What do you need?", func() {
			for _, tip := range []string{
				"Type an app, file, snippet, or emoji name",
				"Calculate 12 * 8, 5 ft to cm, 100 usd to eur; = lists past results",
				"Ask time in tokyo or next friday + 2 weeks",
				"Generate password, or search the web with g rust",
				"? lists every keyword; ↑ brings back a past search",
			} {
				muted(c, p, "• "+tip)
			}
		})
	}
}

func empty(c *ui.Context, p kit.Palette, title string, children func()) {
	ui.Column(c).Grow(1).Padding(24).Gap(12).Justify(ui.Center).Children(func() {
		ui.Text(c, title).Font(kit.DisplayFont).FontSize(24).TextColor(p.Ink).Role(ui.RoleHeading)
		children()
	})
}

func muted(c *ui.Context, p kit.Palette, text string) ui.Element {
	return ui.Text(c, text).TextColor(p.Muted)
}

// row shows a result: its icon, title, subtitle, and on the right a pin,
// its kind while selected, and the key that runs it.
func (m *Model) row(c *ui.Context, p kit.Palette, r core.Result, i int) {
	chosen := i == m.selected
	row := ui.Row(c).Height(52).Padding(0, 12).Gap(12).AlignItems(ui.Center).Radius(10).
		Role(ui.RoleOption).Label(r.Title).AccessSelected(chosen)
	title := p.Ink
	if chosen {
		row.Background(p.Selected).Border(1, p.Edge)
		title = p.SelectedInk
	}
	row.Children(func() {
		m.icon(c, p, r, 32)
		ui.Column(c).Grow(1).Gap(1).Children(func() {
			t := ui.Text(c, r.Title).TextColor(title).SingleLine()
			if chosen {
				t.FontWeight(600)
			}
			if r.Subtitle != "" {
				ui.Text(c, r.Subtitle).FontSize(12).TextColor(p.Muted).SingleLine()
			}
		})
		ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
			if r.Pinned {
				ui.Icon(c, glyphs["pin"]).FontSize(13).TextColor(p.Muted).Label("Pinned")
			}
			if chosen {
				ui.Text(c, kinds[r.Kind].label).FontSize(12).TextColor(p.Muted)
			}
			if i < 9 {
				keyCaps(c, p, modKey, strconv.Itoa(i+1))
			}
		})
	})
	// Moving the pointer onto a row selects it. Only a real move counts:
	// mygo sends InputPointerMove only when the pointer's position in the
	// window changed, never for a list that scrolls, or new results that
	// come, under a still pointer, so those never take the selection from
	// the keyboard.
	row.HandleInput(func(ev ui.InputEvent) bool {
		if ev.Kind == ui.InputPointerMove && i < len(m.rows()) {
			m.selected = i
		}
		return false
	})
	// A click runs the row the user sees; a right-click opens its actions.
	row.OnClick(func() {
		m.selected = i
		m.runRow(i, 0)
		m.focusInput()
	})
	if row.RightClicked() {
		m.selected = i
		m.openMenu()
	}
}

// icon is a file's system icon, an emoji, a color swatch, or a glyph, in a
// square of size: 32 on a row, 56 in the details.
func (m *Model) icon(c *ui.Context, p kit.Palette, r core.Result, size float32) {
	scale := size / 32
	ui.Row(c).Size(size, size).Justify(ui.Center).AlignItems(ui.Center).Children(func() {
		if r.Icon.File != "" {
			if b := m.backend.FileIcon(r.Icon.File); b != nil {
				ui.Image(c, b).Size(size, size)
				return
			}
		}
		switch {
		case r.Icon.Emoji != "":
			ui.Text(c, r.Icon.Emoji).FontSize(26 * scale)
		case r.Icon.Color != "":
			ui.Box(c).Size(24*scale, 24*scale).Radius(6*scale).Background(ui.Hex(r.Icon.Color)).Border(1, p.Border)
		default:
			name := string(r.Icon.Symbol)
			if glyphs[name] == nil {
				name = kinds[r.Kind].glyph
			}
			if svg := glyphs[name]; svg != nil {
				ui.Icon(c, svg).FontSize(19 * scale).TextColor(p.Muted)
			}
		}
	})
}

// footer shows what Enter runs and the Actions button, which it returns:
// a press on it must toggle the menu, not close it as a press outside.
func (m *Model) footer(c *ui.Context, p kit.Palette) ui.Element {
	var actions ui.Element
	m.dragArea(ui.Row(c).Height(40).Padding(0, 16).Gap(12).AlignItems(ui.Center).BorderWidth(1, 0, 0, 0).BorderColor(p.Rule)).Children(func() {
		if r, ok := m.selectedRow(); ok && len(r.Actions) > 0 {
			ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
				ui.Text(c, r.Actions[0].Label).FontSize(12).TextColor(p.Ink)
				keyCaps(c, p, "↩")
			})
		}
		ui.Box(c).Grow(1)
		if m.canClearHistory() {
			ownPress(footerButton(c, p, "Clear History…", nil)).OnClick(func() {
				m.menu = nil
				m.start(clearHistory, "", "")
			})
		}
		actions = ownPress(footerButton(c, p, "Actions", []string{modKey, "K"})).OnClick(func() {
			if m.menu != nil {
				m.closeMenu()
			} else {
				m.openMenu()
			}
		})
	})
	return actions
}

func footerButton(c *ui.Context, p kit.Palette, label string, keys []string) ui.Element {
	b := ui.ButtonBase(c).Padding(4, 8).Radius(6).Label(label)
	if b.Hovered() {
		b.Background(p.Hover)
	}
	return b.Children(func() {
		ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
			ui.Text(c, label).FontSize(12).FontWeight(600).TextColor(p.Muted)
			if keys != nil {
				keyCaps(c, p, keys...)
			}
		})
	})
}

// keyCaps shows the keys of a shortcut, such as ⌘ K.
func keyCaps(c *ui.Context, p kit.Palette, keys ...string) {
	ui.Row(c).Gap(3).Children(func() {
		for _, k := range keys {
			ui.Text(c, k).FontSize(11).Padding(0, 5).Radius(4).Border(1, p.Border).TextColor(p.Muted)
		}
	})
}
