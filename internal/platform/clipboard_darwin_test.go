package platform

import (
	"testing"

	"github.com/ebitengine/purego/objc"
)

// A private pasteboard counts writes as the general one does, and the test
// never touches the user's clipboard.
func TestChangeCountFollowsWrites(t *testing.T) {
	if err := appKit(); err != nil {
		t.Fatal(err)
	}
	pasteboard := objc.ID(objc.GetClass("NSPasteboard")).Send(objc.RegisterName("pasteboardWithUniqueName"))
	defer pasteboard.Send(objc.RegisterName("releaseGlobally"))
	before, ok := changeCount(pasteboard)
	if !ok {
		t.Fatal("no change count")
	}
	pasteboard.Send(objc.RegisterName("clearContents"))
	after, _ := changeCount(pasteboard)
	if after == before {
		t.Fatalf("change count stayed %d after a write", before)
	}
	again, _ := changeCount(pasteboard)
	if again != after {
		t.Fatalf("change count moved from %d to %d without a write", after, again)
	}
	if _, ok := ClipboardChangeCount(); !ok {
		t.Fatal("the general pasteboard has no change count")
	}
}
