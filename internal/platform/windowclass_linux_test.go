package platform

import (
	"os"
	"testing"
	"time"
	"unsafe"

	"github.com/ebitengine/purego"
)

// A shown window's class is the desktop file's StartupWMClass. GTK would
// otherwise capitalize the program name, and docks that compare the class
// would not group the window with the launcher icon.
func TestTheShownWindowUsesTheDesktopClass(t *testing.T) {
	if os.Getenv("DISPLAY") == "" && os.Getenv("WAYLAND_DISPLAY") == "" {
		t.Skip("no display")
	}
	gtk, err := purego.Dlopen("libgtk-3.so.0", purego.RTLD_NOW|purego.RTLD_GLOBAL)
	if err != nil {
		t.Skip("libgtk-3 is not installed: " + err.Error())
	}
	var (
		initCheck   func(argc, argv uintptr) bool
		windowNew   func(typ int32) uintptr
		setTitle    func(w uintptr, title *byte)
		show        func(w uintptr)
		destroy     func(w uintptr)
		getWindow   func(w uintptr) uintptr
		iterate     func() bool
		x11XID      func(w uintptr) uintptr
		openDisplay func(name uintptr) uintptr
		classHint   func(dpy, win uintptr, hint *xClassHint) int32
	)
	purego.RegisterLibFunc(&initCheck, gtk, "gtk_init_check")
	purego.RegisterLibFunc(&windowNew, gtk, "gtk_window_new")
	purego.RegisterLibFunc(&setTitle, gtk, "gtk_window_set_title")
	purego.RegisterLibFunc(&show, gtk, "gtk_widget_show")
	purego.RegisterLibFunc(&destroy, gtk, "gtk_widget_destroy")
	purego.RegisterLibFunc(&getWindow, gtk, "gtk_widget_get_window")
	purego.RegisterLibFunc(&iterate, gtk, "gtk_main_iteration")
	if !initCheck(0, 0) {
		t.Skip("GTK could not open a display")
	}
	// GTK replaces a class set before it starts. The app calls
	// AlignWindowClass from WhenReady, which is this order.
	if err := AlignWindowClass(); err != nil {
		t.Fatal(err)
	}
	gdk, err := purego.Dlopen("libgdk-3.so.0", purego.RTLD_NOW|purego.RTLD_GLOBAL)
	if err != nil {
		t.Fatal(err)
	}
	x11, err := purego.Dlopen("libX11.so.6", purego.RTLD_NOW|purego.RTLD_GLOBAL)
	if err != nil {
		t.Fatal(err)
	}
	purego.RegisterLibFunc(&x11XID, gdk, "gdk_x11_window_get_xid")
	purego.RegisterLibFunc(&openDisplay, x11, "XOpenDisplay")
	purego.RegisterLibFunc(&classHint, x11, "XGetClassHint")

	win := windowNew(0) // GTK_WINDOW_TOPLEVEL
	title := append([]byte("TinyDash class test"), 0)
	setTitle(win, &title[0])
	show(win)
	defer destroy(win)

	var xid uintptr
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		iterate()
		if gw := getWindow(win); gw != 0 {
			xid = x11XID(gw)
			if xid != 0 {
				break
			}
		}
	}
	if xid == 0 {
		t.Skip("the window has no X11 id (a Wayland display)")
	}
	dpy := openDisplay(0)
	if dpy == 0 {
		t.Fatal("XOpenDisplay failed")
	}
	var hint xClassHint
	if classHint(dpy, xid, &hint) == 0 {
		t.Fatal("XGetClassHint failed")
	}
	got := goCString(hint.class)
	if got != DesktopWindowClass {
		t.Fatalf("WM_CLASS class %q, want %q (instance %q)", got, DesktopWindowClass, goCString(hint.name))
	}
}

// xClassHint is XClassHint. The fields are the strings XGetClassHint allocates.
type xClassHint struct {
	name, class *byte
}

func goCString(p *byte) string {
	if p == nil {
		return ""
	}
	n := 0
	// SAFETY: p is a C string XGetClassHint allocated, NUL-terminated,
	// and live until XFree. The test reads it before anything frees it.
	for *(*byte)(unsafe.Add(unsafe.Pointer(p), n)) != 0 {
		n++
	}
	return unsafe.String(p, n)
}
