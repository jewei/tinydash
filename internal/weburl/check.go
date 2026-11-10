package weburl

import (
	"errors"
	"strconv"
	"strings"
)

// Check refuses a filled http, https, or mailto URL that a browser could
// not open, with the WHATWG URL Standard's reasons, as the Rust app's url
// crate did. Only the host and port can fail: a browser encodes anything
// else. Unicode hosts pass unchecked.
func Check(url string) error {
	// Browsers drop tabs and newlines anywhere, and spaces and controls at
	// either end.
	url = strings.Map(func(r rune) rune {
		if r == '\t' || r == '\n' || r == '\r' {
			return -1
		}
		return r
	}, url)
	url = strings.TrimFunc(url, func(r rune) bool { return r <= ' ' })
	scheme, rest, _ := strings.Cut(url, ":")
	if strings.EqualFold(scheme, "mailto") {
		return nil
	}
	// http and https read extra slashes and backslashes after the scheme
	// as one.
	rest = strings.TrimLeft(rest, `/\`)
	authority := rest
	if end := strings.IndexAny(rest, `/\?#`); end >= 0 {
		authority = rest[:end]
	}
	if at := strings.LastIndexByte(authority, '@'); at >= 0 {
		authority = authority[at+1:]
	}
	host, port := authority, ""
	if strings.HasPrefix(authority, "[") {
		end := strings.IndexByte(authority, ']')
		if end < 0 || !ipv6Chars(authority[1:end]) || end == 1 {
			return errors.New("invalid IPv6 address")
		}
		host, port = authority[:end+1], authority[end+1:]
		if port != "" && port[0] != ':' {
			return errors.New("invalid port number")
		}
		port = strings.TrimPrefix(port, ":")
	} else if colon := strings.IndexByte(authority, ':'); colon >= 0 {
		host, port = authority[:colon], authority[colon+1:]
	}
	if host == "" {
		return errors.New("empty host")
	}
	if _, ok := parsePort(port); port != "" && !ok {
		return errors.New("invalid port number")
	}
	if strings.HasPrefix(host, "[") {
		return nil
	}
	host = percentDecode(host)
	if strings.ContainsFunc(host, forbiddenHost) {
		return errors.New("invalid domain character")
	}
	if endsInNumber(host) && !ipv4(host) {
		return errors.New("invalid IPv4 address")
	}
	return nil
}

// percentDecode decodes %XX sequences; others stay as written.
func percentDecode(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] == '%' && i+2 < len(s) {
			if n, err := strconv.ParseUint(s[i+1:i+3], 16, 8); err == nil {
				b.WriteByte(byte(n))
				i += 2
				continue
			}
		}
		b.WriteByte(s[i])
	}
	return b.String()
}

// endsInNumber reports whether a host's last label is a number, which
// makes browsers read the host as an IPv4 address.
func endsInNumber(host string) bool {
	labels := strings.Split(host, ".")
	if labels[len(labels)-1] == "" && len(labels) > 1 {
		labels = labels[:len(labels)-1]
	}
	last := labels[len(labels)-1]
	if last != "" && strings.Trim(last, "0123456789") == "" {
		return true
	}
	_, ok := ipv4Number(last)
	return ok
}

// ipv4 reports whether a host is an IPv4 address in any form browsers
// accept: one to four numbers in decimal, octal (leading 0), or hex (0x).
func ipv4(host string) bool {
	labels := strings.Split(host, ".")
	if labels[len(labels)-1] == "" && len(labels) > 1 {
		labels = labels[:len(labels)-1]
	}
	if len(labels) > 4 {
		return false
	}
	for i, label := range labels {
		n, ok := ipv4Number(label)
		if !ok {
			return false
		}
		limit := uint64(255)
		if i == len(labels)-1 {
			limit = 1<<(8*(5-len(labels))) - 1
		}
		if n > limit {
			return false
		}
	}
	return true
}

func ipv4Number(label string) (uint64, bool) {
	if label == "" {
		return 0, false
	}
	base := 10
	switch {
	case len(label) >= 2 && (label[:2] == "0x" || label[:2] == "0X"):
		label, base = label[2:], 16
	case len(label) >= 2 && label[0] == '0':
		label, base = label[1:], 8
	}
	if label == "" {
		return 0, true
	}
	n, err := strconv.ParseUint(label, base, 64)
	return n, err == nil
}
