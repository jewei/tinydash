package platform

import (
	"runtime"
	"testing"

	"github.com/ebitengine/purego/objc"
)

// On a private pasteboard: the text and the concealed marker, nothing else.
func TestASecretCarriesTheConcealedMarker(t *testing.T) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	if err := appKit(); err != nil {
		t.Fatal(err)
	}
	pasteboard := objc.ID(objc.GetClass("NSPasteboard")).Send(objc.RegisterName("pasteboardWithUniqueName"))
	defer pasteboard.Send(objc.RegisterName("releaseGlobally"))
	writeSecret(pasteboard, "hunter2")
	text := goString(pasteboard.Send(objc.RegisterName("stringForType:"), nsString("public.utf8-plain-text")))
	types := pasteboard.Send(objc.RegisterName("types"))
	n := objc.Send[int](types, objc.RegisterName("count"))
	var names []string
	for i := range n {
		names = append(names, goString(types.Send(objc.RegisterName("objectAtIndex:"), uintptr(i))))
	}
	concealed := false
	for _, name := range names {
		concealed = concealed || name == "org.nspasteboard.ConcealedType"
	}
	if text != "hunter2" || !concealed {
		t.Fatalf("text %q, types %q", text, names)
	}
}
