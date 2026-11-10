// Package weburl parses, writes, and checks web URLs by the WHATWG URL
// Standard, as the Rust app's url crate did. It is shared because features
// may not import each other.
package weburl

import (
	"fmt"
	"strconv"
	"strings"
)

// URL is an HTTP(S) URL split into its parts. Parse and String follow the
// WHATWG URL Standard for the URLs people paste: scheme and host in lower
// case, no default port, a "/" path when there is none, dot segments
// resolved, and the characters that browsers encode percent-encoded.
// Unicode hosts stay as typed instead of becoming punycode.
type URL struct {
	Scheme   string
	Userinfo string // with its "@"
	Host     string
	Port     string // with its ":"
	Path     string
	Query    string
	HasQuery bool
	Fragment string
	HasHash  bool
}

// Characters that are percent-encoded besides controls and non-ASCII.
const (
	fragmentSet = " \"<>`"
	pathSet     = fragmentSet + "#?{}"
	querySet    = " \"#<>'"
)

// Parse reads an http or https URL. ok is false for any other scheme and
// for a host or port that a browser refuses.
func Parse(input string) (u URL, ok bool) {
	scheme, rest, ok := strings.Cut(input, "://")
	scheme = strings.ToLower(scheme)
	if !ok || scheme != "http" && scheme != "https" {
		return URL{}, false
	}
	u = URL{Scheme: scheme}
	// Browsers read extra slashes and backslashes after the scheme as one.
	rest = strings.TrimLeft(rest, "/\\")
	rest, u.Fragment, u.HasHash = strings.Cut(rest, "#")
	rest, u.Query, u.HasQuery = strings.Cut(rest, "?")
	authority := rest
	if i := strings.IndexAny(rest, "/\\"); i >= 0 {
		authority, u.Path = rest[:i], rest[i:]
	}
	if i := strings.LastIndexByte(authority, '@'); i >= 0 {
		u.Userinfo, authority = authority[:i+1], authority[i+1:]
	}
	if !u.setHostPort(authority) {
		return URL{}, false
	}
	u.Path = removeDotSegments(strings.ReplaceAll(u.Path, "\\", "/"))
	return u, true
}

func (u *URL) setHostPort(authority string) bool {
	host, port := authority, ""
	if strings.HasPrefix(authority, "[") {
		end := strings.IndexByte(authority, ']')
		if end < 0 || !ipv6Chars(authority[1:end]) {
			return false
		}
		host, port = authority[:end+1], authority[end+1:]
		if port != "" && port[0] != ':' {
			return false
		}
	} else if i := strings.LastIndexByte(authority, ':'); i >= 0 {
		host, port = authority[:i], authority[i:]
	}
	if host == "" {
		return false
	}
	if !strings.HasPrefix(host, "[") && strings.ContainsFunc(host, forbiddenHost) {
		return false
	}
	u.Host = asciiLower(host)
	if port = strings.TrimPrefix(port, ":"); port != "" {
		n, ok := parsePort(port)
		if !ok {
			return false
		}
		if !(u.Scheme == "http" && n == 80 || u.Scheme == "https" && n == 443) {
			u.Port = ":" + strconv.FormatUint(n, 10)
		}
	}
	return true
}

func (u URL) String() string {
	var b strings.Builder
	b.WriteString(u.Scheme + "://" + u.Userinfo + u.Host + u.Port)
	b.WriteString(encode(u.Path, pathSet))
	if u.HasQuery {
		b.WriteString("?" + encode(u.Query, querySet))
	}
	if u.HasHash {
		b.WriteString("#" + encode(u.Fragment, fragmentSet))
	}
	return b.String()
}

// removeDotSegments resolves "." and ".." (also written %2e) as browsers do.
// An empty path becomes "/".
func removeDotSegments(path string) string {
	segments := strings.Split(strings.TrimPrefix(path, "/"), "/")
	var out []string
	for i, segment := range segments {
		last := i == len(segments)-1
		switch strings.ToLower(segment) {
		case "..", ".%2e", "%2e.", "%2e%2e":
			if len(out) > 0 {
				out = out[:len(out)-1]
			}
			if last {
				out = append(out, "")
			}
		case ".", "%2e":
			if last {
				out = append(out, "")
			}
		default:
			out = append(out, segment)
		}
	}
	return "/" + strings.Join(out, "/")
}

// encode percent-encodes controls, non-ASCII bytes, and the bytes in set.
// An existing escape is kept.
func encode(text, set string) string {
	var b strings.Builder
	for i := 0; i < len(text); i++ {
		c := text[i]
		if c < 0x20 || c >= 0x7f || strings.IndexByte(set, c) >= 0 {
			fmt.Fprintf(&b, "%%%02X", c)
		} else {
			b.WriteByte(c)
		}
	}
	return b.String()
}

func asciiLower(s string) string {
	return strings.Map(func(r rune) rune {
		if 'A' <= r && r <= 'Z' {
			return r + 'a' - 'A'
		}
		return r
	}, s)
}

// forbiddenHost reports a character that a host may not have.
func forbiddenHost(r rune) bool {
	return r <= ' ' || r == 0x7f || strings.ContainsRune(`#%/:<>?@[\]^|`, r)
}

func ipv6Chars(inner string) bool {
	return strings.Trim(inner, "0123456789abcdefABCDEF:.") == ""
}

// parsePort reads a port of decimal digits only, up to 65535.
func parsePort(port string) (uint64, bool) {
	if strings.Trim(port, "0123456789") != "" {
		return 0, false
	}
	n, err := strconv.ParseUint(port, 10, 16)
	return n, err == nil
}
