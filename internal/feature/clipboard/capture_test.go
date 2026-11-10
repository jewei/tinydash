package clipboard

import (
	"bytes"
	"image"
	"image/png"
	"slices"
	"testing"
)

func TestFindsSecretMarkers(t *testing.T) {
	cases := []struct {
		formats []string
		marked  bool
	}{
		{nil, false},
		{[]string{"public.utf8-plain-text"}, false},
		{[]string{"public.utf8-plain-text", "org.nspasteboard.ConcealedType"}, true},
		{[]string{"com.apple.is-sensitive"}, true},
		{[]string{"text/plain", "x-kde-passwordManagerHint"}, true},
		{[]string{"ExcludeClipboardContentFromMonitorProcessing"}, true},
		{[]string{"Clipboard Viewer Ignore"}, true},
		// Its value decides, which Marked does not read.
		{[]string{HistoryFormat}, false},
	}
	for _, c := range cases {
		if got := Marked(c.formats); got != c.marked {
			t.Errorf("Marked(%q) = %v, want %v", c.formats, got, c.marked)
		}
	}
}

func TestMarkerFormatsCoverEveryMarker(t *testing.T) {
	markers := MarkerFormats()
	if !Marked(markers) || !slices.Contains(markers, HistoryFormat) {
		t.Errorf("MarkerFormats() = %q", markers)
	}
	markers[0] = "changed"
	if MarkerFormats()[0] == "changed" {
		t.Error("MarkerFormats shares its slice")
	}
}

func TestAnUnreadableHistoryValueKeepsTheCopyOut(t *testing.T) {
	cases := []struct {
		value []byte
		out   bool
	}{
		{nil, true},
		{[]byte{1, 0}, true},
		{[]byte{0, 0, 0, 0}, true},
		{[]byte{1, 0, 0, 0}, false},
		// Allocator padding after the value.
		{[]byte{1, 0, 0, 0, 0, 0, 0, 0}, false},
		{[]byte{0, 0, 0, 0, 7, 7, 7, 7}, true},
	}
	for _, c := range cases {
		if got := KeptOutOfHistory(c.value); got != c.out {
			t.Errorf("KeptOutOfHistory(%v) = %v, want %v", c.value, got, c.out)
		}
	}
}

func TestKnowsThePasswordApps(t *testing.T) {
	for id, want := range map[string]bool{
		"com.apple.Passwords":      true,
		"com.apple.keychainaccess": true,
		"com.apple.Safari":         false,
		"":                         false,
	} {
		if PasswordApp(id) != want {
			t.Errorf("PasswordApp(%q) = %v", id, !want)
		}
	}
}

func TestACopySeenAfterLeavingAPasswordAppIsSecret(t *testing.T) {
	// Passwords in front, no new copy yet.
	secret, last := look{7, false}.next(7, true)
	if secret {
		t.Error("no new copy")
	}
	// The user copied, then switched to Safari before the next look.
	if secret, last = last.next(8, false); !secret {
		t.Error("copy made before leaving Passwords")
	}
	// Safari stays in front; nothing new.
	if secret, last = last.next(8, false); secret {
		t.Error("same copy, Safari in front")
	}
	// A copy made in Safari is not secret.
	if secret, _ = last.next(9, false); secret {
		t.Error("copy made in Safari")
	}
}

func TestACopyMadeInAPasswordAppIsSecret(t *testing.T) {
	secret, last := look{7, false}.next(8, true)
	if !secret {
		t.Error("copy made in Passwords")
	}
	if secret, _ = last.next(8, true); secret {
		t.Error("the same copy seen again is not new")
	}
}

func TestTheFirstLookJudgesOnlyTheAppInFrontNow(t *testing.T) {
	if secret, _ := noLook.next(3, false); secret {
		t.Error("first look, no password app")
	}
	if secret, _ := noLook.next(3, true); !secret {
		t.Error("first look, password app in front")
	}
}

// Looks keeps a password copy secret for every later reader, as long as it
// is still on the clipboard.
func TestLooksRememberAPasswordCopy(t *testing.T) {
	steps := []struct {
		change      uint64
		passwordApp bool
		secret      bool
	}{
		{5, false, false}, // first look: Safari
		{5, true, true},   // Passwords in front: anything read now is secret
		{6, false, true},  // copied in Passwords, then left it
		{6, false, true},  // a later read of that copy, Safari in front
		{7, false, false}, // a copy made in Safari
		{7, false, false},
	}
	var looks Looks
	for i, s := range steps {
		if got := looks.Look(s.change, s.passwordApp); got != s.secret {
			t.Errorf("step %d: Look(%d, %v) = %v, want %v", i, s.change, s.passwordApp, got, s.secret)
		}
	}
}

func TestLooksStartWithNoLook(t *testing.T) {
	var looks Looks
	if !looks.Look(0, true) {
		t.Error("a first copy made in Passwords")
	}
	if !looks.Look(0, false) {
		t.Error("the same copy after leaving Passwords")
	}
}

func TestChoosesWhatToKeep(t *testing.T) {
	files := []string{"/a"}
	all := Capture{Images: true, Files: true}
	cases := []struct {
		name    string
		capture Capture
		offer   Offer
		want    Kind
	}{
		{"nothing", all, Offer{}, ""},
		{"files before their names as text", all, Offer{Files: files, Text: true}, KindFiles},
		{"files not opted into are not saved as text", Capture{Images: true}, Offer{Files: files, Text: true}, ""},
		{"text before an image", all, Offer{Text: true, Image: true}, KindText},
		{"text without opt-ins", Capture{}, Offer{Text: true}, KindText},
		{"image", all, Offer{Image: true}, KindImage},
		{"image not opted into", Capture{Files: true}, Offer{Image: true}, ""},
		{"a URL list with no local file falls through to text", all, Offer{Files: nil, Text: true}, KindText},
	}
	for _, c := range cases {
		if got := c.capture.Choose(c.offer); got != c.want {
			t.Errorf("%s: Choose = %q, want %q", c.name, got, c.want)
		}
	}
}

func TestReadsThePNGSizeFromItsHeader(t *testing.T) {
	encode := func(width, height int) []byte {
		var b bytes.Buffer
		if err := png.Encode(&b, image.NewGray(image.Rect(0, 0, width, height))); err != nil {
			t.Fatal(err)
		}
		return b.Bytes()
	}
	small := encode(3, 2)
	got, ok := PNGImage(small)
	if !ok || got.Kind != KindImage || got.Width != 3 || got.Height != 2 || !bytes.Equal(got.PNG, small) {
		t.Errorf("PNGImage(3×2) = %+v, %v", got, ok)
	}
	if _, ok := PNGImage(encode(5000, 5000)); ok {
		t.Error("25 megapixels kept")
	}
	if _, ok := PNGImage([]byte("not a png")); ok {
		t.Error("not a PNG kept")
	}
	if _, ok := PNGImage(append(small, make([]byte, MaxImageBytes)...)); ok {
		t.Error("over the byte limit kept")
	}
}
