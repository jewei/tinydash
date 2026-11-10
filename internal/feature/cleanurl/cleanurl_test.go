package cleanurl

import "testing"

func TestClean(t *testing.T) {
	for _, tc := range []struct{ in, want string }{
		{"https://example.com/a?utm_source=x&id=1&q=a%26b&fbclid=y#top", "https://example.com/a?id=1&q=a%26b#top"},
		{"https://www.youtube.com/watch?v=abc&t=42&si=share", "https://www.youtube.com/watch?v=abc&t=42"},
		{"https://shop.example?utm_campaign=x", "https://shop.example/"},
		{"https://www.amazon.de/dp/B0?tag=aff&th=1", "https://www.amazon.de/dp/B0?th=1"},
		{"HTTPS://Example.COM:443/a/./b/../c?UTM_Source=x&id=1", "https://example.com/a/c?id=1"},
		{"http://example.com:8080/café?utm_source=x&q=\"a\"#x y", ""},
		{"http://example.com:8080/café?utm_source=x&q=\"a\"#é", "http://example.com:8080/caf%C3%A9?q=%22a%22#%C3%A9"},
		{"https://example.com/?utm%5Fsource=x&a+b=1&fbclid", "https://example.com/?a+b=1"},
		{"https://[::1]:443/?gclid=1", "https://[::1]/"},
		{"https://example.com/?id=1", ""},
		{"https://example.com/", ""},
		{"https://example.com/?", ""},
		{"example.com/?utm_source=x", ""},
		{"see https://example.com/?utm_source=x", ""},
		{"https://example.com/?tag=keep", ""},
		{"https://?utm_source=x", ""},
		{"https://exa<mple.com/?utm_source=x", ""},
		{"https://example.com:99999/?utm_source=x", ""},
		{"ftp://example.com/?utm_source=x", ""},
	} {
		got, ok := clean(tc.in)
		if got != tc.want || ok != (tc.want != "") {
			t.Errorf("clean(%q) = %q, %v; want %q", tc.in, got, ok, tc.want)
		}
	}
}
