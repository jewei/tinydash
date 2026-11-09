package platform

import (
	"errors"
	"path/filepath"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
	"github.com/ebitengine/purego/objc"
)

var foundation = sync.OnceValues(func() (uintptr, error) {
	return purego.Dlopen("/System/Library/Frameworks/Foundation.framework/Foundation", purego.RTLD_GLOBAL|purego.RTLD_NOW)
})

// constant reads an NSString constant that Foundation exports.
func constant(name string) (objc.ID, error) {
	lib, err := foundation()
	if err != nil {
		return 0, err
	}
	addr, err := purego.Dlsym(lib, name)
	if err != nil {
		return 0, err
	}
	// SAFETY: addr is the address of a global NSString pointer that lives
	// as long as the process.
	return **(**objc.ID)(unsafe.Pointer(&addr)), nil
}

func nsString(s string) objc.ID {
	text := append([]byte(s), 0)
	return objc.ID(objc.GetClass("NSString")).Send(objc.RegisterName("stringWithUTF8String:"), &text[0])
}

func goString(s objc.ID) string {
	if s == 0 {
		return ""
	}
	p := objc.Send[*byte](s, objc.RegisterName("UTF8String"))
	if p == nil {
		return ""
	}
	n := 0
	for *(*byte)(unsafe.Add(unsafe.Pointer(p), n)) != 0 {
		n++
	}
	return string(unsafe.Slice(p, n))
}

// DiskSpace reports the volume that holds path.
func DiskSpace(path string) (Volume, error) {
	if !filepath.IsAbs(path) {
		return Volume{}, errors.New("TinyDash could not find your home folder.")
	}
	var keys [3]objc.ID
	for i, name := range []string{"NSURLVolumeLocalizedNameKey", "NSURLVolumeTotalCapacityKey", "NSURLVolumeAvailableCapacityForImportantUsageKey"} {
		key, err := constant(name)
		if err != nil {
			return Volume{}, err
		}
		keys[i] = key
	}
	pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
	defer pool.Send(objc.RegisterName("drain"))
	url := objc.ID(objc.GetClass("NSURL")).Send(objc.RegisterName("fileURLWithPath:"), nsString(path))
	array := objc.ID(objc.GetClass("NSArray")).Send(objc.RegisterName("arrayWithObjects:count:"), &keys[0], uintptr(len(keys)))
	var failure objc.ID
	values := url.Send(objc.RegisterName("resourceValuesForKeys:error:"), array, &failure)
	if values == 0 {
		return Volume{}, errors.New(goString(failure.Send(objc.RegisterName("localizedDescription"))))
	}
	number := func(key objc.ID) (uint64, bool) {
		value := values.Send(objc.RegisterName("objectForKey:"), key)
		if value == 0 {
			return 0, false
		}
		return objc.Send[uint64](value, objc.RegisterName("unsignedLongLongValue")), true
	}
	total, ok1 := number(keys[1])
	free, ok2 := number(keys[2])
	if !ok1 || !ok2 {
		return Volume{}, errors.New("macOS did not report the disk size.")
	}
	name := goString(values.Send(objc.RegisterName("objectForKey:"), keys[0]))
	if name == "" {
		name = "Disk"
	}
	return Volume{Name: name, TotalBytes: total, FreeBytes: free}, nil
}
