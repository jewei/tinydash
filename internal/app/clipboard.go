package app

import (
	"slices"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/transfer"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/platform"
)

// clipboardOS is the system clipboard as capture uses it. Tests replace it
// with a fake, so they never touch the user's clipboard.
type clipboardOS struct {
	// onMain runs fn on the main thread and waits for it.
	onMain func(fn func())
	// count is the OS change counter; ok is false where there is none.
	count func() (n uint64, ok bool)
	// frontApp is the bundle ID of the app in front, or "".
	frontApp func() string
	// nativeFormats lists the offered formats by their OS names, which the
	// secret markers use, without reading any of them.
	nativeFormats func() []string
	// formats lists the offered formats by mygo's portable names.
	formats func() []transfer.Format
	// read copies the requested formats that the clipboard offers, all from
	// one copy, or fails.
	read func(formats ...transfer.Format) (transfer.Data, error)
	// rich is false where history keeps text only.
	rich bool
}

// Clipboard reads the system clipboard for clipboard history and for a
// quicklink's {clipboard}, and never returns a copy that its source marked
// secret.
//
// mygo's clipboard is main-thread state. Count, Read, and Text run their
// whole check-then-read on the main thread with mygo.RunOnMain and wait for
// it, so no other main-thread clipboard use can come between the secret
// check and the read. Call them from a goroutine, such as the monitor's
// ticker: on the main thread they also work, but an image read then holds up
// the UI. Never call them from a goroutine that the main thread waits for,
// which would deadlock. After the app quits they return nothing.
type Clipboard struct {
	os      clipboardOS
	capture func() clipboard.Capture
	// looks is used on the main thread only.
	looks clipboard.Looks
}

// NewClipboard reads the system clipboard. capture returns the user's
// current image and file opt-ins.
func NewClipboard(capture func() clipboard.Capture) *Clipboard {
	return &Clipboard{
		os: clipboardOS{
			onMain:        mygo.RunOnMain,
			count:         platform.ClipboardChangeCount,
			frontApp:      platform.FrontmostAppID,
			nativeFormats: mygo.Clipboard.AvailableFormats,
			formats:       mygo.Clipboard.Formats,
			read:          mygo.Clipboard.Read,
			rich:          platform.RichClipboard,
		},
		capture: capture,
	}
}

// Count is the monitor's count function. It also notes which app is in
// front at every tick, also while history is off: a copy made in a password
// app just before the user left it is secret only by what the tick before
// saw. Call it too when the launcher opens, for a copy made just before.
func (c *Clipboard) Count() (n uint64, ok bool) {
	c.os.onMain(func() {
		n, ok = c.os.count()
		if ok {
			c.looks.Look(n, clipboard.PasswordApp(c.os.frontApp()))
		}
	})
	return n, ok
}

// Read is the monitor's read function: the content of the latest copy. ok
// is false when there is nothing to keep, such as a copy that its source
// marked secret, a kind the user did not opt into, or content over the
// limits.
func (c *Clipboard) Read() (content clipboard.Content, ok bool) {
	content, ok = c.readOnMain(c.capture())
	return content, ok && content.Fits()
}

// Text is the clipboard's text now, for a quicklink's {clipboard}. ok is
// false when there is none, or its source marked it secret. History need
// not be on.
func (c *Clipboard) Text() (string, bool) {
	content, ok := c.readOnMain(clipboard.Capture{})
	return content.Text, ok && content.Kind == clipboard.KindText
}

func (c *Clipboard) readOnMain(capture clipboard.Capture) (content clipboard.Content, ok bool) {
	c.os.onMain(func() { content, ok = c.read(capture) })
	return content, ok
}

func (c *Clipboard) read(capture clipboard.Capture) (clipboard.Content, bool) {
	before, counted := c.os.count()
	if c.concealed(before, counted) {
		return clipboard.Content{}, false
	}
	content, ok := c.readContent(capture)
	// A copy that replaced the checked one during the read was not checked.
	// Without a counter (Linux), each read brings its own copy's markers.
	if after, _ := c.os.count(); !ok || after != before {
		return clipboard.Content{}, false
	}
	return content, true
}

// concealed checks the copy before any of its content is read.
func (c *Clipboard) concealed(change uint64, counted bool) bool {
	// Look first and always: the look must be noted even for a marked copy.
	if counted && c.looks.Look(change, clipboard.PasswordApp(c.os.frontApp())) {
		return true
	}
	native := c.os.nativeFormats()
	if clipboard.Marked(native) {
		return true
	}
	if !slices.Contains(native, clipboard.HistoryFormat) {
		return false
	}
	data, err := c.os.read(clipboard.HistoryFormat)
	if err != nil {
		return true
	}
	value, _ := data.Read(clipboard.HistoryFormat)
	return clipboard.KeptOutOfHistory(value)
}

func (c *Clipboard) readContent(capture clipboard.Capture) (clipboard.Content, bool) {
	offered := c.os.formats()
	offer := clipboard.Offer{
		Text:  slices.Contains(offered, transfer.Text),
		Image: slices.Contains(offered, transfer.PNG),
	}
	if !c.os.rich {
		capture = clipboard.Capture{}
	} else if slices.Contains(offered, transfer.FileList) || slices.Contains(offered, transfer.URIList) {
		// A list that cannot be read, or holds only web URLs, is no copied
		// files: the copy may still be text.
		if data, ok := c.readChecked(transfer.FileList, transfer.URIList); ok {
			offer.Files, _ = data.Files()
		}
	}
	switch capture.Choose(offer) {
	case clipboard.KindFiles:
		return clipboard.Files(offer.Files), true
	case clipboard.KindText:
		text, ok := c.readFormat(transfer.Text)
		return clipboard.Text(string(text)), ok
	case clipboard.KindImage:
		png, ok := c.readFormat(transfer.PNG)
		if !ok {
			return clipboard.Content{}, false
		}
		return clipboard.PNGImage(png)
	}
	return clipboard.Content{}, false
}

func (c *Clipboard) readFormat(format transfer.Format) ([]byte, bool) {
	data, ok := c.readChecked(format)
	if !ok {
		return nil, false
	}
	b, err := data.Read(format)
	return b, err == nil
}

// readChecked reads formats together with the secret markers, in one read
// of one copy, and fails when that copy is secret. On Linux, which has no
// change counter, this is what keeps a copy made after the first check from
// being read unchecked.
func (c *Clipboard) readChecked(formats ...transfer.Format) (transfer.Data, bool) {
	for _, marker := range clipboard.MarkerFormats() {
		formats = append(formats, transfer.Format(marker))
	}
	data, err := c.os.read(formats...)
	if err != nil {
		return transfer.Data{}, false
	}
	var got []string
	for _, f := range data.Formats() {
		got = append(got, string(f))
	}
	if clipboard.Marked(got) {
		return transfer.Data{}, false
	}
	if slices.Contains(got, clipboard.HistoryFormat) {
		value, _ := data.Read(clipboard.HistoryFormat)
		if clipboard.KeptOutOfHistory(value) {
			return transfer.Data{}, false
		}
	}
	return data, true
}
