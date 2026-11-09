package weburl

import "testing"

func TestParseAndString(t *testing.T) {
	for _, tc := range []struct{ in, want string }{
		{"https://shop.example?a", "https://shop.example/?a"},
		{"HTTPS://Example.COM:443/a/./b/../c?id=1", "https://example.com/a/c?id=1"},
		{"http://example.com:8080/café?q=\"a\"#é", "http://example.com:8080/caf%C3%A9?q=%22a%22#%C3%A9"},
		{"http://example.com:80/a/%2e%2E/b/.", "http://example.com/b/"},
		{`https:///\example.com\a\..\b`, "https://example.com/b"},
		{"https://u:p@example.com/", "https://u:p@example.com/"},
		{"https://[::1]:443/?x", "https://[::1]/?x"},
		{"https://example.com/?", "https://example.com/?"},
		{"https://example.com/#", "https://example.com/#"},
		{"https://?x", ""},
		{"https://exa<mple.com/", ""},
		{"https://example.com:99999/", ""},
		{"https://example.com:8x/", ""},
		{"https://[::g]/", ""},
		{"https://[::1]x/", ""},
		{"example.com/", ""},
		{"ftp://example.com/", ""},
	} {
		u, ok := Parse(tc.in)
		if ok != (tc.want != "") || ok && u.String() != tc.want {
			t.Errorf("Parse(%q) = %q, %v; want %q", tc.in, u.String(), ok, tc.want)
		}
	}
}
