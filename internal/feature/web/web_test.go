package web

import (
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func opened(t *testing.T, r core.Result) string {
	t.Helper()
	open, ok := r.Actions[0].Action.(core.OpenURL)
	if !ok {
		t.Fatalf("unexpected action %#v", r.Actions[0].Action)
	}
	return open.URL
}

func TestKeywordsNeedSearchText(t *testing.T) {
	for _, q := range []string{"gh", "gh  ", "ghostty settings", ""} {
		if _, ok := Answer(q); ok {
			t.Errorf("%q is a search", q)
		}
	}
	r, ok := Answer("GH tauri app")
	if !ok || opened(t, r) != "https://github.com/search?q=tauri%20app" {
		t.Errorf("GH tauri app: %v %#v", ok, r)
	}
}

func TestEncodesTheTextAsOneValue(t *testing.T) {
	if got, want := Fill("https://x.test/?q={query}", "a&b=c 東京"), "https://x.test/?q=a%26b%3Dc%20%E6%9D%B1%E4%BA%AC"; got != want {
		t.Errorf("Fill = %q, want %q", got, want)
	}
	if url := opened(t, Fallback("rust", DuckDuckGo)); !strings.HasPrefix(url, "https://duckduckgo.com/") {
		t.Errorf("DuckDuckGo fallback opens %q", url)
	}
	if url := opened(t, Fallback(" rust ", "")); url != "https://www.google.com/search?q=rust" {
		t.Errorf("default fallback opens %q", url)
	}
}

func TestEveryFallbackEngineHasARow(t *testing.T) {
	for engine, row := range fallbackEngines {
		if !strings.EqualFold(engines[row].Name, string(engine)) {
			t.Errorf("%s maps to %s", engine, engines[row].Name)
		}
	}
}
