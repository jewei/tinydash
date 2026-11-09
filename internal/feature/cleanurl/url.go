package cleanurl

import (
	"fmt"
	"strconv"
	"strings"
)

// url is an HTTP(S) URL split into the parts that cleaning needs. The Rust
// app parsed and wrote URLs by the WHATWG URL Standard, as browsers do; parse
// and String follow it for the URLs people paste: scheme and host in lower
// case, no default port, a "/" path when there is none, dot segments
// resolved, and the characters that browsers encode percent-encoded.
// Unicode hosts stay as typed instead of becoming punycode.
type url struct {
	scheme   string
	userinfo string // with its "@"
	host     string
	port     string // with its ":"
	path     string
	query    string
	hasQuery bool
	fragment string
	hasHash  bool
}

// Characters that are percent-encoded besides controls and non-ASCII.
const (
	fragmentSet = " \"<>`"
	pathSet     = fragmentSet + "#?{}"
	querySet    = " \"#<>'"
)

// Characters a host may not have.
const forbiddenHost = " #%/:<>?@[\\]^|"

func parse(input string) (url, bool) {
	scheme, rest, ok := strings.Cut(input, "://")
	scheme = strings.ToLower(scheme)
	if !ok || scheme != "http" && scheme != "https" {
		return url{}, false
	}
	u := url{scheme: scheme}
	// Browsers read extra slashes and backslashes after the scheme as one.
	rest = strings.TrimLeft(rest, "/\\")
	rest, u.fragment, u.hasHash = strings.Cut(rest, "#")
	rest, u.query, u.hasQuery = strings.Cut(rest, "?")
	authority := rest
	if i := strings.IndexAny(rest, "/\\"); i >= 0 {
		authority, u.path = rest[:i], rest[i:]
	}
	if i := strings.LastIndexByte(authority, '@'); i >= 0 {
		u.userinfo, authority = authority[:i+1], authority[i+1:]
	}
	if !u.setHostPort(authority) {
		return url{}, false
	}
	u.path = removeDotSegments(strings.ReplaceAll(u.path, "\\", "/"))
	return u, true
}

func (u *url) setHostPort(authority string) bool {
	host, port := authority, ""
	if strings.HasPrefix(authority, "[") {
		end := strings.IndexByte(authority, ']')
		if end < 0 || strings.Trim(authority[1:end], "0123456789abcdefABCDEF:.") != "" {
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
	if !strings.HasPrefix(host, "[") && strings.ContainsFunc(host, func(r rune) bool {
		return r < 0x20 || r == 0x7f || strings.ContainsRune(forbiddenHost, r)
	}) {
		return false
	}
	u.host = asciiLower(host)
	if port = strings.TrimPrefix(port, ":"); port != "" {
		if strings.Trim(port, "0123456789") != "" {
			return false
		}
		n, err := strconv.ParseUint(port, 10, 16)
		if err != nil {
			return false
		}
		if !(u.scheme == "http" && n == 80 || u.scheme == "https" && n == 443) {
			u.port = ":" + strconv.FormatUint(n, 10)
		}
	}
	return true
}

func (u url) String() string {
	var b strings.Builder
	b.WriteString(u.scheme + "://" + u.userinfo + u.host + u.port)
	b.WriteString(encode(u.path, pathSet))
	if u.hasQuery {
		b.WriteString("?" + encode(u.query, querySet))
	}
	if u.hasHash {
		b.WriteString("#" + encode(u.fragment, fragmentSet))
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
