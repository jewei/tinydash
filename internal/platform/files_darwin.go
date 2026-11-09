package platform

import (
	"os"
	"syscall"
)

// PackageExtensions are folder extensions that Finder shows as one file;
// the file index lists them without their contents.
var PackageExtensions = []string{"app", "appex", "bundle", "framework", "plugin", "kext", "xpc", "photoslibrary", "musiclibrary", "tvlibrary", "imovielibrary", "fcpbundle", "logicx", "band", "rtfd", "sparsebundle", "xcarchive", "xcodeproj", "xcworkspace", "playground"}

// HiddenCheck is the OS's check for a hidden item besides a leading dot:
// the UF_HIDDEN flag that chflags sets.
func HiddenCheck() func(path string) bool {
	return func(path string) bool {
		info, err := os.Lstat(path)
		if err != nil {
			return false
		}
		stat, ok := info.Sys().(*syscall.Stat_t)
		return ok && stat.Flags&0x8000 != 0
	}
}
