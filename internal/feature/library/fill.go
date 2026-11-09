package library

import (
	"errors"
	"fmt"
	"path/filepath"
	"slices"
	"strings"
	"time"
)

// Target is where a quicklink goes once its placeholders are filled:
// exactly one of URL and Path is set.
type Target struct {
	URL  string
	Path string
}

// QuicklinkTarget fills a quicklink template. Only web and mail URLs and
// absolute or `~` paths are allowed, so a quicklink cannot start a program
// by URL scheme. clipboard, which may be nil, is called only when the
// template has {clipboard}. home replaces a leading `~`.
func QuicklinkTarget(template, query string, now time.Time, home string, clipboard func() string) (Target, error) {
	lower := strings.ToLower(template)
	if strings.HasPrefix(lower, "http://") || strings.HasPrefix(lower, "https://") || strings.HasPrefix(lower, "mailto:") {
		url, _ := fill(template, query, now, clipboard, func(value string) (string, error) { return encode(value), nil })
		if err := checkURL(url); err != nil {
			return Target{}, fmt.Errorf("Invalid quicklink URL: %w", err)
		}
		return Target{URL: url}, nil
	}
	// A value fills in a name; it must not climb out of the folder.
	name := func(value string) (string, error) {
		if strings.ContainsAny(value, `/\`) || slices.Contains(strings.Fields(value), "..") {
			return "", errors.New("A path quicklink accepts a name, not a path.")
		}
		return value, nil
	}
	// `~` comes from the template alone, so no value can pick the home folder.
	template = expandHome(template, home)
	filled, err := fill(template, query, now, clipboard, name)
	if err != nil {
		return Target{}, err
	}
	// The root (a drive, a UNC server and share, or `/`) comes from the
	// template alone: a value must not pick another disk or server.
	if root(template) != root(filled) {
		return Target{}, errors.New("A path quicklink starts with a fixed folder, such as ~/ or /.")
	}
	if moves(template, filled) {
		return Target{}, errors.New("A path quicklink accepts a name, not “.” or “..”.")
	}
	if !filepath.IsAbs(filled) {
		return Target{}, errors.New("A quicklink opens an http, https, or mailto URL, or an absolute or ~ path.")
	}
	return Target{Path: filled}, nil
}

// moves reports whether a value changed a part of the path so that it moves
// up or stays put, as `.{query}` with `.` would. A value holds no separator,
// so the parts of the template and of the filled path line up. Windows also
// drops trailing dots and spaces. An empty part also stays put, which
// matters only before a `..` of the template: that `..` would then climb
// higher than the user wrote. A `..` the template itself has is the user's
// own choice.
func moves(template, filled string) bool {
	parts, done := splitParts(template), splitParts(filled)
	n := min(len(parts), len(done))
	for i := range n {
		if parts[i] == done[i] {
			continue
		}
		staysPut := done[i] != "" && strings.TrimRight(done[i], ". ") == ""
		if staysPut || done[i] == "" && slices.Contains(parts[i+1:n], "..") {
			return true
		}
	}
	return false
}

// splitParts splits a path at both separators on every OS, keeping empty
// parts.
func splitParts(s string) []string {
	var out []string
	start := 0
	for i, r := range s {
		if r == '/' || r == '\\' {
			out = append(out, s[start:i])
			start = i + 1
		}
	}
	return append(out, s[start:])
}

// root is the volume (a drive or a UNC server and share) and the root
// separator that start a path.
func root(path string) string {
	volume := filepath.VolumeName(path)
	rest := path[len(volume):]
	if rest != "" && isSeparator(rest[0]) {
		return volume + string(filepath.Separator)
	}
	return volume
}

// expandHome replaces a leading `~` or `~/` with home.
func expandHome(path, home string) string {
	rest, ok := strings.CutPrefix(path, "~")
	switch {
	case !ok:
		return path
	case rest == "":
		return home
	case rest[0] != '/' && rest[0] != '\\':
		return path
	}
	return joinPath(home, rest[1:])
}

// joinPath appends name to dir as Rust's Path::join does: a rooted name
// replaces dir.
func joinPath(dir, name string) string {
	switch {
	case dir == "" || filepath.IsAbs(name) || name != "" && isSeparator(name[0]):
		return name
	case isSeparator(dir[len(dir)-1]):
		return dir + name
	}
	return dir + string(filepath.Separator) + name
}

func isSeparator(c byte) bool { return c == '/' || c == filepath.Separator }

// RenderSnippet fills a snippet's placeholders. clipboard, which may be
// nil, is called only when the text has {clipboard}.
func RenderSnippet(text, query string, now time.Time, clipboard func() string) string {
	filled, _ := fill(text, query, now, clipboard, func(value string) (string, error) { return value, nil })
	return filled
}

// fill replaces each placeholder in one pass, so that a value never fills
// in turn: a query of `{clipboard}` stays that text. Unknown names stay as
// written. value encodes or checks each value before it goes in.
func fill(template, query string, now time.Time, clipboard func() string, value func(string) (string, error)) (string, error) {
	var b strings.Builder
	copied, read := "", false
	rest := template
	for {
		start := strings.IndexByte(rest, '{')
		if start < 0 {
			break
		}
		b.WriteString(rest[:start])
		rest = rest[start:]
		name, _, closed := strings.Cut(rest[1:], "}")
		var text string
		switch {
		case closed && name == "query":
			text = query
		case closed && name == "date":
			text = now.Format("2006-01-02")
		case closed && name == "time":
			text = now.Format("15:04")
		case closed && name == "datetime":
			text = now.Format("2006-01-02 15:04")
		case closed && name == "clipboard":
			if !read && clipboard != nil {
				copied = clipboard()
			}
			read = true
			text = copied
		default:
			b.WriteByte('{')
			rest = rest[1:]
			continue
		}
		filled, err := value(text)
		if err != nil {
			return "", err
		}
		b.WriteString(filled)
		// The name and its two braces.
		rest = rest[len(name)+2:]
	}
	b.WriteString(rest)
	return b.String(), nil
}

// encode percent-encodes a value so it stays one value in a URL path or
// query string, as the web package does: a space becomes %20, never +.
func encode(text string) string {
	var b strings.Builder
	for i := 0; i < len(text); i++ {
		c := text[i]
		if 'a' <= c && c <= 'z' || 'A' <= c && c <= 'Z' || '0' <= c && c <= '9' || strings.IndexByte("-._~", c) >= 0 {
			b.WriteByte(c)
		} else {
			fmt.Fprintf(&b, "%%%02X", c)
		}
	}
	return b.String()
}
