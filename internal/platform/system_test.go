package platform

import "testing"

func TestRunFailureNamesTheReason(t *testing.T) {
	for _, tc := range []struct{ stderr, want string }{
		{"", "pmset failed. Try again."},
		{"  \n", "pmset failed. Try again."},
		{"Not permitted...\n", "pmset failed: Not permitted. Try again."},
		{"bad argument", "pmset failed: bad argument. Try again."},
	} {
		if got := runFailure("pmset", tc.stderr).Error(); got != tc.want {
			t.Errorf("runFailure(%q) = %q, want %q", tc.stderr, got, tc.want)
		}
	}
}
