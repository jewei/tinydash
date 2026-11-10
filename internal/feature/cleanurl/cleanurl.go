// Package cleanurl removes tracking parameters from a pasted HTTP(S) URL.
// It works offline and never follows the link.
package cleanurl

import (
	"slices"
	"strconv"
	"strings"
	"unicode"

	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/weburl"
)

const maxLength = 8192

// trackers only identify a campaign or click, on any site.
var trackers = []string{
	"fbclid", "gclid", "dclid", "gbraid", "wbraid", "msclkid", "igshid", "mc_cid", "mc_eid",
	"_hsenc", "_hsmi", "yclid", "twclid", "ttclid", "vero_id", "oly_anon_id", "oly_enc_id",
}

// siteTrackers apply on their domains and the subdomains of them.
var siteTrackers = []struct{ domains, keys []string }{
	{
		[]string{
			"amazon.com", "amazon.co.uk", "amazon.de", "amazon.fr", "amazon.it", "amazon.es",
			"amazon.ca", "amazon.co.jp", "amazon.in", "amazon.com.au", "amazon.sg", "amazon.nl",
		},
		[]string{
			"tag", "linkcode", "linkid", "ref", "ref_", "ascsubtag", "creative", "creativeasin",
			"camp", "adid", "sr", "qid", "sprefix", "crid", "dib", "dib_tag", "pd_rd_w", "pd_rd_r",
			"pd_rd_wg", "pf_rd_p", "pf_rd_r",
		},
	},
	{[]string{"youtube.com", "youtu.be"}, []string{"si", "feature", "pp", "ab_channel"}},
	{[]string{"spotify.com"}, []string{"si", "nd", "dlsi"}},
	{[]string{"twitter.com", "x.com"}, []string{"s", "t"}},
	{[]string{"instagram.com"}, []string{"igsh"}},
}

// Answer is the query without its tracking parameters, when it is an
// HTTP(S) URL that has some.
func Answer(query string) (core.Result, bool) {
	cleaned, ok := clean(query)
	if !ok {
		return core.Result{}, false
	}
	return core.Result{
		ID:       core.ID("url:" + cleaned),
		Kind:     core.KindURL,
		Title:    cleaned,
		Subtitle: "Removed tracking parameters",
		Icon:     core.SymbolIcon(core.SymbolLink),
		Actions: []core.ResultAction{
			core.Do("Copy Clean URL", core.Copy{Text: cleaned}),
			core.Do("Open Clean URL", core.OpenURL{URL: cleaned}),
		},
	}, true
}

func clean(input string) (string, bool) {
	input = strings.TrimSpace(input)
	if len(input) > maxLength || strings.IndexFunc(input, unicode.IsSpace) >= 0 {
		return "", false
	}
	u, ok := weburl.Parse(input)
	if !ok || !u.HasQuery {
		return "", false
	}
	var site []string
	for _, s := range siteTrackers {
		if slices.ContainsFunc(s.domains, func(domain string) bool { return onDomain(u.Host, domain) }) {
			site = append(site, s.keys...)
		}
	}
	tracking := func(key string) bool {
		key = asciiLower(percentDecode(key))
		return strings.HasPrefix(key, "utm_") || slices.Contains(trackers, key) || slices.Contains(site, key)
	}
	// Untouched pairs stay byte for byte, so encoded values survive.
	var kept []string
	total := 0
	for pair := range strings.SplitSeq(u.Query, "&") {
		if pair == "" {
			continue
		}
		total++
		key, _, _ := strings.Cut(pair, "=")
		if !tracking(key) {
			kept = append(kept, pair)
		}
	}
	if len(kept) == total {
		return "", false
	}
	u.HasQuery = len(kept) > 0
	u.Query = strings.Join(kept, "&")
	return u.String(), true
}

func onDomain(host, domain string) bool {
	return host == domain || strings.HasSuffix(host, "."+domain)
}

// percentDecode decodes a form key as browsers do: a plus is a space, and a
// broken escape stays as it is.
func percentDecode(text string) string {
	var out strings.Builder
	for i := 0; i < len(text); i++ {
		switch c := text[i]; {
		case c == '+':
			out.WriteByte(' ')
		case c == '%' && i+2 < len(text) && isHex(text[i+1]) && isHex(text[i+2]):
			n, _ := strconv.ParseUint(text[i+1:i+3], 16, 8)
			out.WriteByte(byte(n))
			i += 2
		default:
			out.WriteByte(c)
		}
	}
	return out.String()
}

func isHex(c byte) bool {
	return '0' <= c && c <= '9' || 'a' <= c && c <= 'f' || 'A' <= c && c <= 'F'
}

func asciiLower(s string) string {
	return strings.Map(func(r rune) rune {
		if 'A' <= r && r <= 'Z' {
			return r + 'a' - 'A'
		}
		return r
	}, s)
}
