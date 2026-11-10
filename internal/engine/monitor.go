package engine

import "github.com/jewei/tinydash/internal/feature/clipboard"

// Monitor saves what the user copies into the clipboard history. The app
// calls Tick on a timer; count and read reach the OS clipboard.
type Monitor struct {
	engine *Engine
	// count is the OS clipboard change counter; ok is false where the OS
	// has none, and the monitor compares the content instead.
	count func() (n uint64, ok bool)
	// read returns the clipboard content, and whether the read found
	// content to keep, nothing to keep, or failed.
	read func() (clipboard.Content, clipboard.Read)

	seen     uint64
	seenHash int64
	started  bool
	// rebase: history was off since the content was last seen, so the next
	// read is only a baseline. Without it, what the user copied while
	// history was off would look like a new copy once history is on.
	rebase bool
	offs   uint64 // the engine's historyOffs at the last tick
}

// NewMonitor returns a monitor that takes what is on the clipboard now as
// already seen, so starting TinyDash does not save it.
func NewMonitor(e *Engine, count func() (uint64, bool), read func() (clipboard.Content, clipboard.Read)) *Monitor {
	return &Monitor{engine: e, count: count, read: read}
}

// Tick saves the clipboard if it changed since the last tick. It returns
// nil when there was nothing to save, or where the save's result arrives.
func (m *Monitor) Tick(now int64) <-chan error {
	n, counted := m.count()
	// History went off since the last tick, even if this monitor never saw
	// it off: a copy made in between must not count as new.
	if offs := m.engine.historyOffs.Load(); offs != m.offs {
		m.offs, m.rebase = offs, true
	}
	enabled := m.engine.current.Load().Settings.ClipboardHistoryEnabled
	if counted && m.started && n == m.seen {
		if enabled {
			// Nothing changed since the content was last seen, so it is the
			// baseline already, and the next change is a real copy.
			m.rebase = false
		}
		return nil
	}
	// The epoch first, then the clipboard: a Clear or a switch-off that
	// lands between the two makes the save stale, and the writer drops it.
	epoch := m.engine.ClipEpoch()
	// A first look takes what is there as seen: at startup, and after
	// history was off where no tick could keep up with the changes.
	first := !m.started || m.rebase
	m.started, m.seen = true, n
	// While history is off, nothing is read: the counter is enough to see
	// a change, and the content of a copy is none of TinyDash's business.
	// With a counter, this tick took the change as seen; without one, the
	// next read must be a baseline.
	if !enabled {
		m.rebase = !counted
		return nil
	}
	m.rebase = false
	content, got := m.read()
	// A copy made during the read means the content may be newer than the
	// counter this tick saw. Keep neither: the next tick sees the new count
	// and reads again, so a saved read always matches its count, and a copy
	// saved once is not saved again after a Clear.
	if after, _ := m.count(); counted && after != n {
		return nil
	}
	// Nothing to keep, or the read failed. Keep what was seen: where the OS
	// has no counter, forgetting it would make the unchanged text look new
	// at the next tick, and a cleared copy would come back. A first look
	// that failed saw nothing, so there the next read that works is the
	// baseline; with a counter, this tick took the change as seen.
	if got != clipboard.ReadContent {
		if got == clipboard.ReadFailed && first && !counted {
			m.rebase = true
		}
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
