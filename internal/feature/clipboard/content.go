// Package clipboard decides what clipboard history keeps, how it names an
// entry, and how the history is searched. Capture belongs to the engine;
// storage belongs to the store.
package clipboard

import (
	"fmt"
	"hash/fnv"
	"path/filepath"
	"strings"
	"unicode/utf8"
)

const (
	// MaxTextBytes: longer text is not saved. Very large copies are
	// usually data, not snippets.
	MaxTextBytes = 16 * 1024
	// MaxImageBytes: larger images are not saved.
	MaxImageBytes  = 8 * 1024 * 1024
	maxImagePixels = 16_000_000
	// MaxFiles is the most file references in one entry.
	MaxFiles = 64
	// MaxImages is the most unpinned images kept, whatever the history
	// limit.
	MaxImages = 32
	// maxTitleChars bounds a text entry's title.
	maxTitleChars = 200
)

// Kind is what an entry holds. The values are the ones tinydash.db stores.
type Kind string

const (
	KindText  Kind = "text"
	KindImage Kind = "image"
	KindFiles Kind = "files"
)

// Content is what the clipboard held at capture time. Only the fields of its
// Kind are set.
type Content struct {
	Kind Kind
	Text string
	// PNG, Width, and Height describe an image.
	PNG           []byte
	Width, Height uint32
	// Files are references to existing files. Their contents are never
	// copied.
	Files []string
}

// Text is copied text.
func Text(text string) Content { return Content{Kind: KindText, Text: text} }

// Image is a copied image, encoded as PNG.
func Image(png []byte, width, height uint32) Content {
	return Content{Kind: KindImage, PNG: png, Width: width, Height: height}
}

// Files is a list of copied files.
func Files(paths []string) Content { return Content{Kind: KindFiles, Files: paths} }

// PixelsFit reports whether an image this size may be saved. Capture checks
// it before it decodes an image, so a huge one never fills memory.
func PixelsFit(width, height uint32) bool {
	return uint64(width)*uint64(height) <= maxImagePixels
}

// Fits reports whether the content may be saved at all. Text and paths must
// be UTF-8, as the Rust app's strings always were, so that tinydash.db holds
// them unchanged.
func (c Content) Fits() bool {
	switch c.Kind {
	case KindText:
		return strings.TrimSpace(c.Text) != "" && len(c.Text) <= MaxTextBytes &&
			!strings.ContainsRune(c.Text, 0) && utf8.ValidString(c.Text)
	case KindImage:
		return len(c.PNG) <= MaxImageBytes && PixelsFit(c.Width, c.Height)
	case KindFiles:
		if len(c.Files) == 0 || len(c.Files) > MaxFiles {
			return false
		}
		for _, path := range c.Files {
			if !filepath.IsAbs(path) || !utf8.ValidString(path) {
				return false
			}
		}
		return true
	}
	return false
}

// Hash is a stable 64-bit FNV-1a hash of the content, used to find repeats.
// tinydash.db stores it, so it never changes.
func (c Content) Hash() int64 {
	h := fnv.New64a()
	switch c.Kind {
	case KindText:
		h.Write([]byte("text"))
		h.Write([]byte(c.Text))
	case KindImage:
		h.Write([]byte("image"))
		h.Write(c.PNG)
	case KindFiles:
		h.Write([]byte("files"))
		for _, path := range c.Files {
			h.Write([]byte(path))
			h.Write([]byte{0})
		}
	}
	return int64(h.Sum64())
}

// Title is one line that names the entry in the list.
func (c Content) Title() string {
	switch c.Kind {
	case KindText:
		for line := range strings.SplitSeq(c.Text, "\n") {
			if line = strings.TrimSpace(line); line != "" {
				return firstChars(line, maxTitleChars)
			}
		}
		return ""
	case KindImage:
		return fmt.Sprintf("Image %d × %d", c.Width, c.Height)
	case KindFiles:
		switch len(c.Files) {
		case 0:
			return ""
		case 1:
			return displayName(c.Files[0])
		default:
			return fmt.Sprintf("%s and %d more", displayName(c.Files[0]), len(c.Files)-1)
		}
	}
	return ""
}

func firstChars(s string, n int) string {
	for i := range s {
		if n == 0 {
			return s[:i]
		}
		n--
	}
	return s
}

// displayName is the last part of a path, or the whole path when it ends
// in no name, as Rust's Path::file_name decides: empty and "." parts after
// the first are skipped, and ".." or a root is no name.
func displayName(path string) string {
	rest := filepath.ToSlash(path[len(filepath.VolumeName(path)):])
	last := ""
	for i, part := range strings.Split(rest, "/") {
		if part == "" || (part == "." && i > 0) {
			continue
		}
		last = part
	}
	if last == "" || last == "." || last == ".." {
		return path
	}
	return last
}

// Entry is one saved entry, without image bytes.
type Entry struct {
	ID    int64
	Kind  Kind
	Title string
	// Text is what search reads besides the title: the text, or the file
	// paths one per line. It is empty for an image.
	Text string
	// CopiedAt is the Unix time of the last copy.
	CopiedAt int64
}
