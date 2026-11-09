package engine

import "github.com/jewei/tinydash/internal/feature/clipboard"

// Monitor saves what the user copies into the clipboard history. The app
// calls Tick on a timer; count and read reach the OS clipboard.
type Monitor struct {
	engine *Engine
	// count is the OS clipboard change counter; ok is false where the OS
	// has none, and the monitor compares the content instead.
	count func() (n uint64, ok bool)
	// read returns the clipboard content; ok is false when it holds
	// nothing to keep, such as a password manager's secret.
	read func() (content clipboard.Content, ok bool)

	seen     uint64
	seenHash int64
	started  bool
}

// NewMonitor returns a monitor that takes what is on the clipboard now as
// already seen, so starting TinyDash does not save it.
func NewMonitor(e *Engine, count func() (uint64, bool), read func() (clipboard.Content, bool)) *Monitor {
	return &Monitor{engine: e, count: count, read: read}
}

// Tick saves the clipboard if it changed since the last tick. It returns
// nil when there was nothing to save, or where the save's result arrives.
func (m *Monitor) Tick(now int64) <-chan error {
	n, counted := m.count()
	if counted && m.started && n == m.seen {
		return nil
	}
	// The epoch first, then the clipboard: a Clear or a switch-off that
	// lands between the two makes the save stale, and the writer drops it.
	epoch := m.engine.ClipEpoch()
	first := !m.started
	m.started, m.seen = true, n
	// While history is off, nothing is read: the counter is enough to see
	// a change, and the content of a copy is none of TinyDash's business.
	if !m.engine.current.Load().Settings.ClipboardHistoryEnabled {
		m.seenHash = 0
		return nil
	}
	content, ok := m.read()
	if !ok {
		return nil
	}
	hash := content.Hash()
	if !counted && hash == m.seenHash && !first {
		return nil
	}
	m.seenHash = hash
	if first {
		return nil
	}
	return m.engine.SaveClip(epoch, content, now)
}
