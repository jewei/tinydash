package platform

// ClipboardChangeCount returns a number that changes whenever any app
// writes the clipboard, so the monitor reads the content only then. ok is
// false where the OS has no such counter: on Linux, clipboard history keeps
// text only, and the monitor compares the text itself.
func ClipboardChangeCount() (count uint64, ok bool) { return 0, false }

// FrontmostAppID returns "": only macOS has password apps that copy
// without a secret marker.
func FrontmostAppID() string { return "" }

// RichClipboard reports whether clipboard history can keep images and
// copied files. Linux keeps text only, as the Rust app did, and as the
// documentation promises Linux users.
const RichClipboard = false
