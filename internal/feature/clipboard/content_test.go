package clipboard

import (
	"strings"
	"testing"
)

func TestDecidesWhatFits(t *testing.T) {
	cases := []struct {
		name    string
		content Content
		fits    bool
	}{
		{"text", Text("hi"), true},
		{"blank text", Text(" \n"), false},
		{"text with NUL", Text("a\x00b"), false},
		{"text at the limit", Text(strings.Repeat("x", MaxTextBytes)), true},
		{"text over the limit", Text(strings.Repeat("x", MaxTextBytes+1)), false},
		{"text that is not UTF-8", Text("S\xe3o"), false},
		{"relative file", Files([]string{"relative"}), false},
		{"no files", Files(nil), false},
		{"absolute file", Files([]string{"/tmp/a.txt"}), true},
		{"too many files", Files(make([]string, MaxFiles+1)), false},
		{"16 megapixels", Image(nil, 4000, 4000), true},
		{"25 megapixels", Image(nil, 5000, 5000), false},
		{"image over the byte limit", Image(make([]byte, MaxImageBytes+1), 1, 1), false},
		{"no kind", Content{}, false},
	}
	for _, c := range cases {
		if got := c.content.Fits(); got != c.fits {
			t.Errorf("%s: fits %v, want %v", c.name, got, c.fits)
		}
	}
}

func TestLimitsPixelsWithoutOverflow(t *testing.T) {
	if !PixelsFit(4000, 4000) || PixelsFit(12_000, 12_000) || PixelsFit(^uint32(0), ^uint32(0)) {
		t.Error("PixelsFit")
	}
}

func TestTitles(t *testing.T) {
	cases := []struct {
		content Content
		want    string
	}{
		{Text("\n  first line \nsecond"), "first line"},
		{Text("\r\n\t\r\nwindows\r\n"), "windows"},
		{Text(strings.Repeat("é", 250)), strings.Repeat("é", 200)},
		{Files([]string{"/a/one.txt", "/a/two.txt"}), "one.txt and 1 more"},
		{Files([]string{"/a/one.txt"}), "one.txt"},
		{Files([]string{"/a/dir/"}), "dir"},
		{Files([]string{"/a/dir/."}), "dir"},
		{Files([]string{"/a/.."}), "/a/.."},
		{Files([]string{"/"}), "/"},
		{Image(nil, 2, 1), "Image 2 × 1"},
	}
	for _, c := range cases {
		if got := c.content.Title(); got != c.want {
			t.Errorf("%+v: title %q, want %q", c.content, got, c.want)
		}
	}
}

func TestHashesTellContentApart(t *testing.T) {
	a, again := Text("a"), Text("a")
	if a.Hash() == Text("b").Hash() || a.Hash() != again.Hash() {
		t.Error("text hashes")
	}
	if Files([]string{"/a", "/b"}).Hash() == Files([]string{"/a/b"}).Hash() {
		t.Error("file lists that join to the same bytes must differ")
	}
}
