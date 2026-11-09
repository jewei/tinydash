package app

import (
	"bytes"
	"errors"
	"image"
	"image/png"
	"slices"
	"testing"

	"github.com/egoist/mygo/transfer"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/golden"
)

// fakeClipboard stands in for the system clipboard, so no test reads or
// writes the user's real one.
type fakeClipboard struct {
	t       *testing.T
	change  uint64
	counted bool
	// native are the formats by OS name; data holds the content by mygo's
	// portable names. A test sets them apart to model a copy that lands
	// between the check and the read.
	native []string
	data   transfer.Data
	front  string
	// readErr fails reads of the formats it names.
	readErr []transfer.Format
	// afterRead runs after each read, to model a copy made during one.
	afterRead func()
	reads     [][]transfer.Format
	inMain    bool
}

func newFake(t *testing.T) *fakeClipboard { return &fakeClipboard{t: t, counted: true} }

// copy puts a new copy on the fake clipboard, as an app's copy would.
func (f *fakeClipboard) copy(native []string, reps ...transfer.Representation) {
	f.change++
	f.native = native
	f.data = transfer.New(transfer.NewItem(reps...))
}

func (f *fakeClipboard) reader(rich bool, capture clipboard.Capture) *Clipboard {
	onMain := func(name string) {
		if !f.inMain {
			f.t.Errorf("%s ran off the main thread", name)
		}
	}
	return &Clipboard{
		os: clipboardOS{
			onMain: func(fn func()) {
				f.inMain = true
				defer func() { f.inMain = false }()
				fn()
			},
			count: func() (uint64, bool) {
				onMain("count")
				if !f.counted {
					return 0, false
				}
				return f.change, true
			},
			frontApp:      func() string { onMain("frontApp"); return f.front },
			nativeFormats: func() []string { onMain("nativeFormats"); return f.native },
			formats:       func() []transfer.Format { onMain("formats"); return f.data.Formats() },
			read: func(formats ...transfer.Format) (transfer.Data, error) {
				onMain("read")
				f.reads = append(f.reads, formats)
				if f.afterRead != nil {
					defer f.afterRead()
				}
				for _, failing := range f.readErr {
					if slices.Contains(formats, failing) && slices.Contains(f.data.Formats(), failing) {
						return transfer.Data{}, errors.New("the clipboard is busy")
					}
				}
				return f.data.Materialize(formats)
			},
			rich: rich,
		},
		capture: func() clipboard.Capture { return capture },
	}
}

// readContent reports whether a read asked for any content format, as
// opposed to markers only.
func (f *fakeClipboard) readContent() bool {
	for _, formats := range f.reads {
		for _, format := range formats {
			if format == transfer.Text || format == transfer.PNG || format == transfer.FileList {
				return true
			}
		}
	}
	return false
}

var (
	macText   = []string{"public.utf8-plain-text"}
	all       = clipboard.Capture{Images: true, Files: true}
	textRep   = func(s string) transfer.Representation { return transfer.Bytes(transfer.Text, []byte(s)) }
	formatRep = func(name string, b []byte) transfer.Representation { return transfer.Bytes(transfer.Format(name), b) }
)

func TestReadsText(t *testing.T) {
	f := newFake(t)
	f.copy(macText, textRep("hello"))
	got, ok := f.reader(true, all).Read()
	if !ok || got.Kind != clipboard.KindText || got.Text != "hello" {
		t.Errorf("Read() = %+v, %v", got, ok)
	}
}

// A secret is never read: the check sees the format names only.
func TestNeverReadsAMarkedCopy(t *testing.T) {
	for _, marker := range []string{"org.nspasteboard.ConcealedType", "com.apple.is-sensitive", "ExcludeClipboardContentFromMonitorProcessing", "Clipboard Viewer Ignore", "x-kde-passwordManagerHint"} {
		f := newFake(t)
		f.copy(append(slices.Clone(macText), marker), textRep("hunter2"), formatRep(marker, nil))
		c := f.reader(true, all)
		if got, ok := c.Read(); ok {
			t.Errorf("%s: Read() = %+v", marker, got)
		}
		if text, ok := c.Text(); ok {
			t.Errorf("%s: Text() = %q", marker, text)
		}
		if f.readContent() {
			t.Errorf("%s: content read: %q", marker, f.reads)
		}
	}
}

func TestWindowsHistoryValueDecides(t *testing.T) {
	cases := []struct {
		name  string
		value []byte
		busy  bool
		keep  bool
	}{
		{"0 keeps it out", []byte{0, 0, 0, 0}, false, false},
		{"1 lets it in", []byte{1, 0, 0, 0}, false, true},
		{"too short to read", []byte{1}, false, false},
		{"busy clipboard", []byte{1, 0, 0, 0}, true, false},
	}
	for _, c := range cases {
		f := newFake(t)
		f.copy([]string{"CF_UNICODETEXT", clipboard.HistoryFormat}, textRep("text"), formatRep(clipboard.HistoryFormat, c.value))
		if c.busy {
			f.readErr = []transfer.Format{clipboard.HistoryFormat}
		}
		_, ok := f.reader(true, all).Read()
		if ok != c.keep {
			t.Errorf("%s: kept %v, want %v", c.name, ok, c.keep)
		}
		if !c.keep && f.readContent() {
			t.Errorf("%s: content read: %q", c.name, f.reads)
		}
	}
}

func TestSkipsCopiesFromPasswordApps(t *testing.T) {
	f := newFake(t)
	f.copy(macText, textRep("copied in Safari"))
	f.front = "com.apple.Safari"
	c := f.reader(true, all)
	c.Count()
	// Passwords in front: anything on the clipboard now may be its copy.
	f.front = "com.apple.Passwords"
	if _, ok := c.Read(); ok {
		t.Error("read while Passwords is in front")
	}
	c.Count()
	// The user copies in Passwords and switches to Safari before the next
	// tick.
	f.copy(macText, textRep("hunter2"))
	f.front = "com.apple.Safari"
	c.Count()
	if got, ok := c.Read(); ok {
		t.Errorf("Read() = %+v", got)
	}
	if text, ok := c.Text(); ok {
		t.Errorf("Text() = %q", text)
	}
	// A copy made in Safari is kept.
	f.copy(macText, textRep("hello"))
	c.Count()
	if got, ok := c.Read(); !ok || got.Text != "hello" {
		t.Errorf("Read() = %+v, %v", got, ok)
	}
}

// The read itself looks too, for a copy newer than the monitor's last tick.
func TestAReadLooksBeforeTheNextTick(t *testing.T) {
	f := newFake(t)
	f.front = "com.apple.keychainaccess"
	c := f.reader(true, all)
	c.Count()
	f.copy(macText, textRep("hunter2"))
	f.front = "com.apple.Safari"
	if text, ok := c.Text(); ok {
		t.Errorf("Text() = %q", text)
	}
}

func TestSkipsACopyMadeDuringTheRead(t *testing.T) {
	f := newFake(t)
	f.copy(macText, textRep("checked"))
	f.afterRead = func() { f.copy(append(slices.Clone(macText), "org.nspasteboard.ConcealedType"), textRep("hunter2")) }
	if got, ok := f.reader(true, all).Read(); ok {
		t.Errorf("Read() = %+v", got)
	}
}

// Without a counter, the read brings the markers of the copy it read.
func TestWithoutACounterTheReadChecksItsOwnCopy(t *testing.T) {
	f := newFake(t)
	f.counted = false
	f.copy([]string{"UTF8_STRING"}, textRep("hunter2"), formatRep("x-kde-passwordManagerHint", []byte("secret")))
	if got, ok := f.reader(false, all).Read(); ok {
		t.Errorf("Read() = %+v", got)
	}
	f.copy([]string{"UTF8_STRING"}, textRep("hello"))
	if got, ok := f.reader(false, all).Read(); !ok || got.Text != "hello" {
		t.Errorf("Read() = %+v, %v", got, ok)
	}
}

func fileList(t *testing.T, paths ...string) []byte {
	d, err := transfer.FileData(paths...)
	if err != nil {
		t.Fatal(err)
	}
	b, err := d.Read(transfer.FileList)
	if err != nil {
		t.Fatal(err)
	}
	return b
}

func TestFilesComeBeforeTheirNames(t *testing.T) {
	path := golden.Abs("/Users/me/report.pdf")
	finder := func(f *fakeClipboard) {
		f.copy([]string{"public.file-url", "public.utf8-plain-text"},
			transfer.Bytes(transfer.FileList, fileList(t, path)), textRep("report.pdf"))
	}
	f := newFake(t)
	finder(f)
	if got, ok := f.reader(true, all).Read(); !ok || got.Kind != clipboard.KindFiles || !slices.Equal(got.Files, []string{path}) {
		t.Errorf("files on: Read() = %+v, %v", got, ok)
	}
	f = newFake(t)
	finder(f)
	if got, ok := f.reader(true, clipboard.Capture{Images: true}).Read(); ok {
		t.Errorf("files off: Read() = %+v, want nothing, not the names as text", got)
	}
	f = newFake(t)
	finder(f)
	if text, ok := f.reader(true, all).Text(); ok {
		t.Errorf("Text() = %q, want none for copied files", text)
	}
	// Linux keeps text only.
	f = newFake(t)
	f.counted = false
	finder(f)
	if got, ok := f.reader(false, all).Read(); !ok || got.Kind != clipboard.KindText || got.Text != "report.pdf" {
		t.Errorf("text only: Read() = %+v, %v", got, ok)
	}
}

func TestAWebURLIsText(t *testing.T) {
	f := newFake(t)
	f.copy([]string{"public.url", "public.utf8-plain-text"},
		transfer.Bytes(transfer.URIList, []byte("https://example.com/")), textRep("https://example.com/"))
	if got, ok := f.reader(true, all).Read(); !ok || got.Kind != clipboard.KindText {
		t.Errorf("Read() = %+v, %v", got, ok)
	}
}

func pngOf(t *testing.T, width, height int) []byte {
	var b bytes.Buffer
	if err := png.Encode(&b, image.NewGray(image.Rect(0, 0, width, height))); err != nil {
		t.Fatal(err)
	}
	return b.Bytes()
}

func TestImages(t *testing.T) {
	small := pngOf(t, 4, 3)
	cases := []struct {
		name    string
		png     []byte
		rich    bool
		capture clipboard.Capture
		keep    bool
	}{
		{"opted in", small, true, all, true},
		{"not opted in", small, true, clipboard.Capture{Files: true}, false},
		{"text only", small, false, all, false},
		{"over the pixel limit", pngOf(t, 5000, 5000), true, all, false},
		{"not a PNG", []byte("not a png"), true, all, false},
	}
	for _, c := range cases {
		f := newFake(t)
		f.copy([]string{"public.tiff"}, transfer.Bytes(transfer.PNG, c.png))
		got, ok := f.reader(c.rich, c.capture).Read()
		if ok != c.keep {
			t.Errorf("%s: kept %v, want %v", c.name, ok, c.keep)
		}
		if ok && (got.Kind != clipboard.KindImage || got.Width != 4 || got.Height != 3) {
			t.Errorf("%s: Read() = %v %d×%d", c.name, got.Kind, got.Width, got.Height)
		}
	}
}

func TestReadKeepsOnlyWhatFits(t *testing.T) {
	f := newFake(t)
	f.copy(macText, textRep("  \n"))
	c := f.reader(true, all)
	if _, ok := c.Read(); ok {
		t.Error("blank text kept")
	}
	// A quicklink takes the clipboard's text as it is.
	if text, ok := c.Text(); !ok || text != "  \n" {
		t.Errorf("Text() = %q, %v", text, ok)
	}
}

func TestAnEmptyClipboardHoldsNothing(t *testing.T) {
	f := newFake(t)
	f.change = 1
	if got, ok := f.reader(true, all).Read(); ok {
		t.Errorf("Read() = %+v", got)
	}
}
