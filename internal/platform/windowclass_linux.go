package platform

import (
	"runtime"

	"github.com/ebitengine/purego"
)

// AlignWindowClass sets the X11 window class (the second WM_CLASS string)
// to DesktopWindowClass. Call it after GTK has started and before any
// window is shown. GTK fills the class with the program name while it
// starts, and capitalizes the first letter, so a program named tinydash
// gets the class Tinydash. A dock compares that with StartupWMClass, and
// the two strings differ. A class set before GTK starts is replaced then.
// A window GTK itself made while starting keeps that capitalized class.
// Windows opened after this call use DesktopWindowClass.
// On other systems this is a no-op.
func AlignWindowClass() error {
	lib, err := purego.Dlopen("libgdk-3.so.0", purego.RTLD_LAZY|purego.RTLD_GLOBAL)
	if err != nil {
		return err
	}
	var set func(*byte)
	purego.RegisterLibFunc(&set, lib, "gdk_set_program_class")
	text := append([]byte(DesktopWindowClass), 0)
	set(&text[0])
	runtime.KeepAlive(text)
	return nil
}
