package launcher

import (
	"strings"

	"github.com/jewei/tinydash/internal/core"
)

// menu is the open actions menu. It keeps the items it showed as it opened,
// so a refresh or a new tab cannot change what Enter runs in it.
type menu struct {
	items  []menuItem
	filter string
	active int
}

type menuItem struct {
	label string
	keys  []string // the keys that run it without the menu, or nil
	run   func()
}

// visible are the items whose label holds every word of the filter.
func (mn *menu) visible() []menuItem {
	words := strings.Fields(strings.ToLower(mn.filter))
	var out []menuItem
items:
	for _, item := range mn.items {
		label := strings.ToLower(item.label)
		for _, w := range words {
			if !strings.Contains(label, w) {
				continue items
			}
		}
		out = append(out, item)
	}
	return out
}

func (m *Model) openMenu() { m.menu = &menu{items: m.menuItems()} }

func (m *Model) closeMenu() {
	m.menu = nil
	m.focusInput()
}

func (m *Model) filterMenu() { m.menu.active = 0 }

func (m *Model) moveMenu(by int) {
	m.menu.active = max(0, min(m.menu.active+by, len(m.menu.visible())-1))
}

// chooseMenu closes the menu, then runs its active item, so a dialog the
// item opens takes the keyboard.
func (m *Model) chooseMenu() {
	items := m.menu.visible()
	if m.menu.active >= len(items) {
		return
	}
	item := items[m.menu.active]
	m.closeMenu()
	item.run()
}

// clearHistory is the System command's action, with its words.
var clearHistory = core.ResultAction{
	Label:   "Clear Clipboard History",
	Action:  core.ClearClipboard{},
	Confirm: "Delete all clipboard history except pinned entries?",
}

// canClearHistory: on the Clipboard tab, clearing it is one key away, not a
// trip to System.
func (m *Model) canClearHistory() bool {
	return m.category == core.CategoryClipboard && !m.clipboardOff()
}

// menuItems are the selected result's actions, then the clipboard card's,
// then the focus timer's, then the actions of no result.
func (m *Model) menuItems() []menuItem {
	var items []menuItem
	if r, ok := m.selectedRow(); ok {
		for i, ra := range r.Actions {
			items = append(items, menuItem{label: ra.Label, keys: shortcutFor(ra.Action, i), run: func() { m.runResult(r, i) }})
		}
	}
	items = append(items, m.cardMenuItems()...)
	for i, a := range m.focusActions() {
		var keys []string
		if i == 0 {
			keys = []string{modKey, "P"}
		}
		items = append(items, menuItem{label: a.Label, keys: keys, run: func() { m.controlFocus(a) }})
	}
	var general []core.ResultAction
	if m.canClearHistory() {
		general = append(general, clearHistory)
	}
	general = append(general,
		core.Do(refreshLabel(m.backend.Settings()), core.Refresh{}),
		core.Do("Center Launcher", core.CenterLauncher{}),
		core.Do("Settings", core.OpenSettings{}),
		core.Do("Quit TinyDash", core.Quit{}),
	)
	for _, ra := range general {
		var keys []string
		if _, ok := ra.Action.(core.OpenSettings); ok {
			keys = []string{modKey, ","}
		}
		items = append(items, menuItem{label: ra.Label, keys: keys, run: func() { m.start(ra, "", "") }})
	}
	return items
}

// refreshLabel names what Refresh downloads: rates only while they are on.
func refreshLabel(s ViewSettings) string {
	if s.CurrencyRates {
		return "Refresh Apps, Files, and Rates"
	}
	return "Refresh Apps and Files"
}

// shortcutFor are the keys that run the action at position i of the
// selected result directly.
func shortcutFor(a core.Action, i int) []string {
	switch {
	case i == 0:
		return []string{"↩"}
	case i == 1:
		return []string{modKey, "↩"}
	case deletesEntry(a):
		return []string{modKey, "⌫"}
	}
	return nil
}
