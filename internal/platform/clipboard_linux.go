package platform

// ClipboardChangeCount returns a number that changes whenever any app
// writes the clipboard, so the monitor reads the content only then. ok is
// false where the OS has no such counter: on Linux, clipboard history keeps
// text only, and the monitor compares the text itself.
func ClipboardChangeCount() (count uint64, ok bool) { return 0, false }
