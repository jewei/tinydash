package platform

import "syscall"

// PackageExtensions: Windows shows no folder as one file.
var PackageExtensions []string

// HiddenCheck is the OS's check for a hidden item besides a leading dot:
// the hidden attribute.
func HiddenCheck() func(path string) bool {
	return func(path string) bool {
		name, err := syscall.UTF16PtrFromString(path)
		if err != nil {
			return false
		}
		attributes, err := syscall.GetFileAttributes(name)
		return err == nil && attributes&syscall.FILE_ATTRIBUTE_HIDDEN != 0
	}
}
