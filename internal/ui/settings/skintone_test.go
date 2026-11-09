package settings

import "testing"

func TestSkinToneRejectsValuesOutsideAUint8(t *testing.T) {
	for _, c := range []struct {
		value string
		tone  uint8
		ok    bool
	}{
		{"0", 0, true},
		{"5", 5, true},
		{"255", 255, true},
		{"256", 0, false},
		{"261", 0, false},
		{"-1", 0, false},
		{"x", 0, false},
	} {
		tone, ok := skinTone(c.value)
		if ok != c.ok || (ok && tone != c.tone) {
			t.Errorf("%q: got %d %v, want %d %v", c.value, tone, ok, c.tone, c.ok)
		}
	}
}
