package launcher

import (
	"strings"

	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// The menu and the dialogs are drawn inside the panel, not as windows or
// native menus: the panel never becomes the key window, and a window or
// menu of its own would take the keyboard from it.

// menuView is the actions menu above the footer. It takes no backdrop, so
// a press on a tab closes it and goes on to the tab.
func (m *Model) menuView(c *ui.Context, p kit.Palette, actions ui.Element) {
	mn := m.menu
	ui.Overlay(c, func() {
		panel := ui.Column(c).Absolute().Right(16).Bottom(48).Width(320).MaxHeight(320).Clip().
			Border(1, p.Border).Radius(10).Background(p.Paper).Shadow(0, 12, 40, 0, p.Shadow).
			Role(ui.RoleDialog).Label("Actions").ObserveInput(m.observe)
		panel.Children(func() {
			filter := ui.TextInputBase(c, &mn.filter).AutoFocus().Padding(12).
				Placeholder("Search actions").Label("Search actions")
			filter.OnTextKey(m.menuKey)
			if filter.Changed() {
				m.filterMenu()
			}
			ui.Divider(c)
			items := mn.visible()
			ui.Column(c).Padding(4).Role(ui.RoleListBox).Label("Actions").Children(func() {
				for i, item := range items {
					m.menuRow(c, p, item, i)
				}
			})
			if len(items) == 0 {
				ui.Text(c, "No matching actions").Padding(0, 12, 8, 12).TextColor(p.Muted).Role(ui.RoleStatus)
			}
		})
		if panel.PressedOutside() && !actions.Hovered() {
			m.closeMenu()
		}
	})
}

func (m *Model) menuRow(c *ui.Context, p kit.Palette, item menuItem, i int) {
	chosen := i == m.menu.active
	row := ui.Row(c).Padding(6, 8).Radius(6).Gap(8).AlignItems(ui.Center).
		Role(ui.RoleOption).Label(item.label).AccessSelected(chosen)
	ink, weight := p.Ink, 400
	if chosen {
		row.Background(p.Selected)
		ink, weight = p.SelectedInk, 600
	}
	// As in a native menu, the item under the pointer is the active one.
	row.HandleInput(func(ev ui.InputEvent) bool {
		if ev.Kind == ui.InputPointerMove && m.menu != nil {
			m.menu.active = i
		}
		return false
	})
	row.Children(func() {
		ui.Text(c, item.label).TextColor(ink).FontWeight(weight).Grow(1).SingleLine()
		if item.keys != nil {
			keyCaps(c, p, item.keys...)
		}
	}).OnClick(func() {
		m.menu.active = i
		m.chooseMenu()
	})
}

// menuKey takes the menu's keys in its filter. A held Mod+K opened the
// menu; its repeats must not close it again.
func (m *Model) menuKey(k ui.TextKey) bool {
	switch {
	case k.Mods == 0 && k.Key == ui.KeyDown:
		m.moveMenu(1)
	case k.Mods == 0 && k.Key == ui.KeyUp:
		m.moveMenu(-1)
	case k.Mods == 0 && k.Key == ui.KeyEnter:
		if !m.repeat {
			m.chooseMenu()
		}
	case k.Mods == 0 && k.Key == ui.KeyEscape, k.Mods == ui.Cmd && k.Key == ui.KeyK:
		if !m.repeat {
			m.closeMenu()
		}
	case k.Key == ui.KeyTab:
		// The filter is the only control: Tab must not leave an open menu.
	default:
		return false
	}
	return true
}

// dialog draws a modal panel over the dimmed launcher. A press outside
// dismisses it; Escape acts as the model's escape does, once while held.
func (m *Model) dialog(c *ui.Context, p kit.Palette, role ui.Role, label string, dismiss func(), children func()) {
	ui.Overlay(c, func() {
		back := ui.Column(c).Absolute().Left(0).Top(0).Right(0).Bottom(0).
			Background(p.Backdrop).Justify(ui.Center).AlignItems(ui.Center).Modal()
		back.Children(func() {
			panel := ui.Column(c).Width(360).Padding(24).Gap(16).Radius(10).Border(1, p.Border).
				Background(p.Paper).Shadow(0, 12, 40, 0, p.Shadow).
				Role(role).Label(label).ObserveInput(m.observe).Children(children)
			if panel.PressedOutside() {
				dismiss()
			}
		})
		if back.OverlayShortcut(0, ui.KeyEscape) && !c.ShortcutRepeated() {
			m.escape()
		}
	})
}

// button runs fn on a click or a new press of Enter or Space: a held Enter
// opened the dialog, and its repeats must not answer it.
func (m *Model) button(b ui.Element, disabled bool, fn func()) {
	b.Disabled(disabled).OnClick(func() {
		if !m.repeat {
			fn()
		}
	})
}

func (m *Model) confirmView(c *ui.Context, p kit.Palette) {
	cf := m.confirm
	m.dialog(c, p, ui.RoleAlertDialog, "Confirm", m.cancelConfirm, func() {
		ui.Text(c, cf.action.Confirm)
		ui.Row(c).Gap(8).Justify(ui.End).Children(func() {
			// Cancel comes first, so it has the focus and Enter is safe.
			m.button(ui.Button(c, "Cancel"), false, m.cancelConfirm)
			m.button(ui.Button(c, cf.action.Label).Background(p.Danger).TextColor(p.DangerInk), false, m.confirmPending)
		})
	})
}

func (m *Model) aliasView(c *ui.Context, p kit.Palette) {
	d := m.alias
	label := "Alias for " + d.title
	m.dialog(c, p, ui.RoleDialog, label, m.cancelAlias, func() {
		ui.Text(c, label)
		ui.TextInput(c, &d.text).Bind(&m.aliasField).AutoFocus().Label(label).OnTextKey(func(k ui.TextKey) bool {
			if k.Key != ui.KeyEnter || k.Mods != 0 {
				return false
			}
			if !m.repeat {
				m.saveAlias()
			}
			return true
		})
		if d.err != "" {
			ui.Text(c, d.err).TextColor(p.Danger).Role(ui.RoleAlert)
		}
		ui.Text(c, "One word. Typing it puts this result first. Leave it empty to remove the alias.").FontSize(12).TextColor(p.Muted)
		ui.Row(c).Gap(8).Justify(ui.End).Children(func() {
			m.button(ui.Button(c, "Cancel"), d.saving, m.cancelAlias)
			m.button(ui.PrimaryButton(c, "Save Alias"), d.saving, m.saveAlias)
		})
	})
}

func (m *Model) hotkeyView(c *ui.Context, p kit.Palette) {
	d := m.hotkey
	label := "Hotkey for " + d.title
	m.dialog(c, p, ui.RoleDialog, label, m.closeHotkey, func() {
		ui.Text(c, label)
		m.recorder(c, p, d, label)
		if d.err != "" {
			ui.Text(c, d.err).TextColor(p.Danger).Role(ui.RoleAlert)
		}
		ui.Text(c, "Press keys with a modifier, such as Control or Alt, or a function key. They run this result from any app. Escape stops recording.").FontSize(12).TextColor(p.Muted)
		ui.Row(c).Gap(8).Justify(ui.End).Children(func() {
			if d.shortcut != "" {
				m.button(ui.Button(c, "Remove Hotkey"), m.savingHotkey, func() { m.saveHotkey("") })
			}
			m.button(ui.Button(c, "Close"), m.savingHotkey, m.closeHotkey)
		})
	})
}

// recorder takes every key while it records, so no key reaches the
// launcher or the dialog's buttons; Escape, or leaving it, as for another
// app, stops it. A click or a new press of Enter or Space starts it again.
func (m *Model) recorder(c *ui.Context, p kit.Palette, d *hotkeyDialog, label string) {
	name := label + ": None"
	if d.recording {
		name = label + ": press the new keys, or Escape to cancel"
	} else if d.shortcut != "" {
		name = label + ": " + strings.Join(kit.DisplayKeys(d.shortcut, mac, kit.SuperName), " ")
	}
	edge := p.Border
	if d.recording {
		edge = p.Accent
	}
	rec := ui.Row(c).Focusable().AutoFocus().MinWidth(140).Height(32).Padding(0, 12).Radius(6).
		Border(1, edge).Background(p.Field).Justify(ui.Center).AlignItems(ui.Center).
		Role(ui.RoleButton).Label(name)
	rec.HandleInput(func(ev ui.InputEvent) bool {
		if ev.Kind != ui.InputKeyDown {
			return false
		}
		switch {
		case d.recording && ev.Key == ui.KeyEscape:
			m.stopRecording("")
		case d.recording:
			if keys := kit.Accelerator(ev, mac); keys != "" {
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
		if d.recording {
			m.stopRecording("")
		} else {
			m.startRecording()
		}
	})
	focused := rec.Focused()
	if d.recording && d.recorderFocused && !focused {
		m.stopRecording("")
	}
	d.recorderFocused = focused
	rec.Children(func() {
		switch {
		case d.recording:
			ui.Text(c, "Press keys…").TextColor(p.Ink)
		case d.shortcut != "":
			keyCaps(c, p, kit.DisplayKeys(d.shortcut, mac, kit.SuperName)...)
		default:
			ui.Text(c, "None").TextColor(p.Muted)
		}
	})
}
