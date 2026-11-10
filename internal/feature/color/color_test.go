package color

import (
	"reflect"
	"slices"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func TestReadsColorsInThreeForms(t *testing.T) {
	for _, text := range []string{"#2f6f5e", " #2F6F5E\n", "rgb(47, 111, 94)", "rgb(47 111 94)", "RGB (47,111,94)", "hsl(164, 41%, 31%)", "hsl(164deg 41% 31%)"} {
		r, ok := Answer(text)
		if !ok {
			t.Errorf("%q is a color", text)
			continue
		}
		if !slices.Contains([]string{"#2F6F5E", "#2F6F5D", "#2F705E"}, r.Title) {
			t.Errorf("%q: %s", text, r.Title)
		}
	}
	r, _ := Answer("#fff")
	if want := "rgb(255, 255, 255) · hsl(0, 0%, 100%) · 1.0:1 on white"; r.Subtitle != want {
		t.Errorf("#fff: %q, want %q", r.Subtitle, want)
	}
	if r, _ := Answer("hsl(0, 100%, 50%)"); r.Title != "#FF0000" {
		t.Errorf("hsl red is %s", r.Title)
	}
	if r, _ := Answer("hsl(-120, 100%, 50%)"); r.Title != "#0000FF" {
		t.Errorf("hsl(-120) is %s", r.Title)
	}
	for _, text := range []string{"#12345", "#ggg", "#", "rgb(300, 0, 0)", "rgb(1, 2)", "rgb(-0, 0, 0)", "hsl(0, 120%, 50%)", "hsl(0x10, 50%, 50%)", "hsl(inf, 50%, 50%)", "red", ""} {
		if _, ok := Answer(text); ok {
			t.Errorf("%q is not a color", text)
		}
	}
}

func TestDecodesHowItWasCopiedAndItsContrast(t *testing.T) {
	for text, want := range map[string]Color{
		"#fff": {Hex: "#FFFFFF", RGB: "rgb(255, 255, 255)", HSL: "hsl(0, 0%, 100%)",
			OnWhite: Contrast{"1.0:1", "Fail"}, OnBlack: Contrast{"21.0:1", "AAA"}},
		"hsl(0, 100%, 50%)": {Hex: "#FF0000", RGB: "rgb(255, 0, 0)", HSL: "hsl(0, 100%, 50%)", CopiedAs: "HSL",
			OnWhite: Contrast{"4.0:1", "AA large"}, OnBlack: Contrast{"5.3:1", "AA"}},
		"rgb(47 111 94)": {Hex: "#2F6F5E", RGB: "rgb(47, 111, 94)", HSL: "hsl(164, 41%, 31%)", CopiedAs: "RGB",
			OnWhite: Contrast{"5.9:1", "AA"}, OnBlack: Contrast{"3.6:1", "AA large"}},
		"#000": {Hex: "#000000", RGB: "rgb(0, 0, 0)", HSL: "hsl(0, 0%, 0%)",
			OnWhite: Contrast{"21.0:1", "AAA"}, OnBlack: Contrast{"1.0:1", "Fail"}},
	} {
		if got, ok := Decode(text); !ok || got != want {
			t.Errorf("%q: %+v, want %+v", text, got, want)
		}
	}
}

func TestATypedColorIsAnAnswer(t *testing.T) {
	r, ok := Answer(" #2f6f5e ")
	if !ok {
		t.Fatal("no answer")
	}
	want := core.Result{
		ID:       "color:#2F6F5E",
		Kind:     core.KindColor,
		Title:    "#2F6F5E",
		Subtitle: "rgb(47, 111, 94) · hsl(164, 41%, 31%) · 5.9:1 on white",
		Icon:     core.ColorIcon("#2F6F5E"),
		Actions: []core.ResultAction{
			core.Do("Copy HEX", core.Copy{Text: "#2F6F5E"}),
			core.Do("Copy RGB", core.Copy{Text: "rgb(47, 111, 94)"}),
			core.Do("Copy HSL", core.Copy{Text: "hsl(164, 41%, 31%)"}),
		},
	}
	if !reflect.DeepEqual(r, want) {
		t.Errorf("got %#v\nwant %#v", r, want)
	}
	for _, q := range []string{"{\"a\": 1}", "755", ""} {
		if _, ok := Answer(q); ok {
			t.Errorf("%q is a color", q)
		}
	}
}
