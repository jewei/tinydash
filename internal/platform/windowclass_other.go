//go:build !linux

package platform

// AlignWindowClass does nothing. Only Linux docks match a window to its
// desktop file through the window class.
func AlignWindowClass() error { return nil }
