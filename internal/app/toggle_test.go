package app

import (
	"testing"
	"time"
)

// A shortcut press that itself blurred the panel hides it; a later press
// shows it again.
func TestTheShortcutThatBlurredThePanelLeavesItHidden(t *testing.T) {
	for since, hidden := range map[time.Duration]bool{
		0:                       true,
		100 * time.Millisecond:  true,
		blurWithin:              false,
		2 * time.Second:         false,
		time.Since(time.Time{}): false, // never hidden by blur
	} {
		if blurWasThisPress(since) != hidden {
			t.Errorf("%v after a blur: stays hidden %v, want %v", since, !hidden, hidden)
		}
	}
}
