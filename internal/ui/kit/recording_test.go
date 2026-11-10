package kit

import (
	"errors"
	"testing"
)

func TestRecordingKeepsItsOwnReason(t *testing.T) {
	saved, resumed := errors.New("save refused"), errors.New("resume refused")
	var r Recording
	var first, second int
	steps := []struct {
		name string
		do   func()
		on   bool
		err  string
	}{
		{"start", func() { first = r.Start() }, true, ""},
		{"save refused", func() { r.Fail(first, saved) }, false, "save refused"},
		{"resume refused", func() { r.Resumed(first, resumed) }, false, "save refused"},
		{"start again", func() { second = r.Start() }, true, ""},
		{"an old reply", func() { r.Fail(first, saved); r.Resumed(first, resumed) }, true, ""},
		{"stop", r.Stop, false, ""},
		{"resume refused", func() { r.Resumed(second, resumed) }, false, "resume refused"},
		{"no failure", func() { r.Fail(second, nil) }, false, "resume refused"},
		{"save", func() { r.Save() }, false, ""},
	}
	for i, s := range steps {
		s.do()
		if r.On() != s.on || r.Err() != s.err {
			t.Fatalf("step %d %s: on %v err %q, want %v %q", i, s.name, r.On(), r.Err(), s.on, s.err)
		}
	}
}
