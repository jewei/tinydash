package platform

import (
	"path/filepath"
	"syscall"
	"unsafe"
)

var getDiskFreeSpaceEx = syscall.NewLazyDLL("kernel32.dll").NewProc("GetDiskFreeSpaceExW")

// DiskSpace reports the volume that holds path.
func DiskSpace(path string) (Volume, error) {
	wide, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return Volume{}, err
	}
	var free, total uint64
	done, _, callErr := getDiskFreeSpaceEx.Call(uintptr(unsafe.Pointer(wide)), uintptr(unsafe.Pointer(&free)), uintptr(unsafe.Pointer(&total)), 0)
	if done == 0 {
		return Volume{}, callErr
	}
	name := filepath.VolumeName(path)
	if name == "" {
		name = "Disk"
	}
	return Volume{Name: name, TotalBytes: total, FreeBytes: free}, nil
}
