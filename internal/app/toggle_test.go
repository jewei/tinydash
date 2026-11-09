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

// A blur from a drag must not hide the panel: it stays while it moves and
// when it has the focus back; only a blur with neither hides it.
func TestABlurFromADragDoesNotHideThePanel(t *testing.T) {
	never := time.Since(time.Time{})
	cases := []struct {
		name                 string
		focused              bool
		sinceMove, sinceDrag time.Duration
		movedSinceDrag       bool
		want                 blurStep
	}{
		{"focus came back", true, never, never, false, stayAfterBlur},
		{"moving", false, 10 * time.Millisecond, never, false, waitForDrag},
		{"drag pressed, no move yet", false, never, 300 * time.Millisecond, false, waitForDrag},
		{"drag ended, focus back", true, time.Second, 2 * time.Second, true, stayAfterBlur},
		{"drag ended, focus elsewhere", false, time.Second, 2 * time.Second, true, hideAfterBlur},
		{"a drag that never moved", false, never, dragLimit, false, hideAfterBlur},
		{"plain blur", false, never, never, false, hideAfterBlur},
	}
	for _, c := range cases {
		if got := blurOutcome(c.focused, c.sinceMove, c.sinceDrag, c.movedSinceDrag); got != c.want {
			t.Errorf("%s: %v, want %v", c.name, got, c.want)
		}
	}
}
