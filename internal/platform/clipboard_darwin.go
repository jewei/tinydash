package platform

import (
	"sync"

	"github.com/ebitengine/purego"
	"github.com/ebitengine/purego/objc"
)

var appKit = sync.OnceValue(func() error {
	_, err := purego.Dlopen("/System/Library/Frameworks/AppKit.framework/AppKit", purego.RTLD_GLOBAL|purego.RTLD_NOW)
	return err
})

// ClipboardChangeCount returns a number that changes whenever any app
// writes the clipboard, so the monitor reads the content only then. ok is
// false where the OS has no such counter (Linux).
func ClipboardChangeCount() (count uint64, ok bool) {
	return changeCount(objc.ID(objc.GetClass("NSPasteboard")).Send(objc.RegisterName("generalPasteboard")))
}

func changeCount(pasteboard objc.ID) (uint64, bool) {
	if appKit() != nil || pasteboard == 0 {
		return 0, false
	}
	return uint64(objc.Send[int](pasteboard, objc.RegisterName("changeCount"))), true
}
