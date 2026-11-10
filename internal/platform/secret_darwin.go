package platform

import "github.com/ebitengine/purego/objc"

// WriteSecretText puts text on the clipboard with the marker that asks
// clipboard managers to skip it (nspasteboard.org's ConcealedType). mygo
// cannot write that type on macOS: it maps custom formats to its own
// types. Call it on the main thread. handled is false where the app must
// write the text and markers itself.
func WriteSecretText(text string) (handled bool, err error) {
	if err := appKit(); err != nil {
		return true, err
	}
	writeSecret(objc.ID(objc.GetClass("NSPasteboard")).Send(objc.RegisterName("generalPasteboard")), text)
	return true, nil
}

func writeSecret(pasteboard objc.ID, text string) {
	pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
	defer pool.Send(objc.RegisterName("drain"))
	pasteboard.Send(objc.RegisterName("clearContents"))
	pasteboard.Send(objc.RegisterName("setString:forType:"), nsString(text), nsString("public.utf8-plain-text"))
	empty := objc.ID(objc.GetClass("NSData")).Send(objc.RegisterName("data"))
	pasteboard.Send(objc.RegisterName("setData:forType:"), empty, nsString("org.nspasteboard.ConcealedType"))
}
