package weburl

import "testing"

func TestCheck(t *testing.T) {
	for url, ok := range map[string]bool{
		"https://x.test/a b":         true,
		"https://":                   false,
		"https:///x":                 true,
		"https://user@":              false,
		"http://localhost:":          true,
		"http://localhost:8080/x":    true,
		"http://localhost:x":         false,
		"http://localhost:65536":     false,
		"https://a%20b.test":         false,
		"https://a b.test":           false,
		"https://[::1]:80/":          true,
		"https://[::1":               false,
		"https://1":                  true,
		"https://1.2.3.4":            true,
		"https://1.2.3.999":          false,
		"https://0x7f.1":             true,
		"https://example.com.":       true,
		"https://東京.test":            true,
		"mailto:":                    true,
		"mailto:me@x.test?subject=a": true,
		"HTTPS://X.TEST":             true,
	} {
		if err := Check(url); (err == nil) != ok {
			t.Errorf("%s: %v", url, err)
		}
	}
}
