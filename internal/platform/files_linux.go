package platform

// PackageExtensions: Linux shows no folder as one file.
var PackageExtensions []string

// HiddenCheck is nil: on Linux only a leading dot hides a file.
func HiddenCheck() func(path string) bool { return nil }
