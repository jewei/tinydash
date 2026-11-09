package platform

import "syscall"

var getClipboardSequenceNumber = syscall.NewLazyDLL("user32.dll").NewProc("GetClipboardSequenceNumber")

// ClipboardChangeCount returns a number that changes whenever any app
// writes the clipboard, so the monitor reads the content only then. ok is
// false where the OS has no such counter (Linux).
func ClipboardChangeCount() (count uint64, ok bool) {
	if getClipboardSequenceNumber.Find() != nil {
		return 0, false
	}
	n, _, _ := getClipboardSequenceNumber.Call()
	// Zero means the process lacks access to the window station.
	return uint64(n), n != 0
}
