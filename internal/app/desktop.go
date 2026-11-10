package app

import (
	"errors"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/transfer"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/platform"
)

// desktop does what actions ask of the OS, through mygo and platform. The
// engine checked each action before it gets here.
type desktop struct {
	clip *Clipboard
}

// LaunchApp launches by path: mygo's Shell.LaunchApp reads only the path
// on every OS, so an app that mygo did not list launches too.
func (d *desktop) LaunchApp(path string) error {
	return mygo.Shell.LaunchApp(mygo.InstalledApp{Path: path})
}

func (d *desktop) OpenPath(path string) error { return mygo.Shell.OpenPath(path) }

func (d *desktop) Reveal(path string) error {
	mygo.Shell.ShowItemInFolder(path)
	return nil
}

func (d *desktop) OpenURL(url string) error { return mygo.Shell.OpenExternal(url) }

// secretMarkers ask clipboard managers to skip a copy on Windows and Linux,
// as arboard's exclude_from_history did for the Rust app. macOS takes its
// marker in platform.WriteSecretText.
var secretMarkers = []transfer.Representation{
	transfer.Bytes("ExcludeClipboardContentFromMonitorProcessing", nil),
	transfer.Bytes("CanIncludeInClipboardHistory", []byte{0, 0, 0, 0}),
	transfer.Bytes("CanUploadToCloudClipboard", []byte{0, 0, 0, 0}),
	transfer.Bytes("x-kde-passwordManagerHint", []byte("secret")),
}

func (d *desktop) WriteText(text string, secret bool) error {
	if !secret {
		return mygo.Clipboard.Write(transfer.TextData(text))
	}
	var handled bool
	var err error
	mygo.RunOnMain(func() { handled, err = platform.WriteSecretText(text) })
	if handled || err != nil {
		return err
	}
	reps := append([]transfer.Representation{transfer.Bytes(transfer.Text, []byte(text))}, secretMarkers...)
	return mygo.Clipboard.Write(transfer.New(transfer.NewItem(reps...)))
}

func (d *desktop) WriteClip(content clipboard.Content) error {
	switch content.Kind {
	case clipboard.KindText:
		return mygo.Clipboard.Write(transfer.TextData(content.Text))
	case clipboard.KindFiles:
		return mygo.Clipboard.WriteFiles(content.Files...)
	case clipboard.KindImage:
		return mygo.Clipboard.WriteImage(content.PNG)
	}
	return errors.New("This clipboard entry has nothing to copy.")
}

func (d *desktop) ClipboardText(secretToo bool) (string, bool) {
	if !secretToo {
		return d.clip.Text()
	}
	var text string
	mygo.RunOnMain(func() { text = mygo.Clipboard.ReadText() })
	return text, text != ""
}

func (d *desktop) RunSystem(command core.SystemCommand) error {
	return platform.RunSystemCommand(command, mygo.Shell.OpenExternal)
}
