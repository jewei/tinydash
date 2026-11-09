package engine

import (
	"io/fs"
	"strconv"
	"time"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/feature/library"
)

// DetailsKind says which part of Details holds what the pane shows.
type DetailsKind uint8

const (
	// DetailsText is a clipboard text or a snippet's text, in Text.
	DetailsText DetailsKind = iota + 1
	// DetailsImage is a copied image, in PNG, Width, and Height.
	DetailsImage
	// DetailsFiles is a list of copied files, in Paths.
	DetailsFiles
	// DetailsFile is an app's, a file's, or a folder's facts, in File.
	DetailsFile
)

// Details are what the details pane shows of a result beyond its row, as
// the Rust app's preview.rs loaded them.
type Details struct {
	Kind          DetailsKind
	Text          string
	PNG           []byte
	Width, Height uint32
	Paths         []string
	File          FileFacts
}

// FileFacts are an app's, a file's, or a folder's place, size, and age.
type FileFacts struct {
	Path string
	Size int64
	// Modified is zero when the disk does not tell.
	Modified time.Time
	IsDir    bool
}

// Lstater reads a path's facts without following a symbolic link, as
// os.Lstat does.
type Lstater interface {
	Lstat(path string) (fs.FileInfo, error)
}

// Details are the details of the result with id, or false when it has
// none: only clipboard entries while history is on, snippets, indexed apps,
// and files that the launcher offered have them, so a view cannot probe
// other paths. It reads the disk and tinydash.db, so it must not run on
// the main thread.
func (a *Actions) Details(id core.ID, disk Lstater) (Details, bool) {
	source, key, ok := core.ParseID(id)
	if !ok {
		return Details{}, false
	}
	s := a.state()
	switch source {
	case core.SourceClip:
		n, err := strconv.ParseInt(key, 10, 64)
		if err != nil {
			return Details{}, false
		}
		content, err := a.clip(n)
		if err != nil {
			return Details{}, false
		}
		switch content.Kind {
		case clipboard.KindImage:
			return Details{Kind: DetailsImage, PNG: content.PNG, Width: content.Width, Height: content.Height}, true
		case clipboard.KindFiles:
			return Details{Kind: DetailsFiles, Paths: content.Files}, true
		}
		return Details{Kind: DetailsText, Text: content.Text}, true
	case core.SourceApp:
		if s.Apps.Contains(key) {
			return fileDetails(key, disk)
		}
	case core.SourceFile:
		if a.offered(s, key) {
			return fileDetails(key, disk)
		}
	case core.SourceSnippet:
		n, err := strconv.ParseInt(key, 10, 64)
		if err != nil {
			return Details{}, false
		}
		if item, ok := s.Library.Find(n); ok && item.Kind == library.KindSnippet {
			return Details{Kind: DetailsText, Text: item.Text}, true
		}
	}
	return Details{}, false
}

func fileDetails(path string, disk Lstater) (Details, bool) {
	info, err := disk.Lstat(path)
	if err != nil {
		return Details{}, false
	}
	return Details{Kind: DetailsFile, File: FileFacts{Path: path, Size: info.Size(), Modified: info.ModTime(), IsDir: info.IsDir()}}, true
}
