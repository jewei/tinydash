// Package color answers a color typed as hex, rgb(), or hsl() with its other
// forms and its contrast on white.
package color

import (
	"fmt"
	"math"
	"strconv"
	"strings"
	"unicode"

	"github.com/jewei/tinydash/internal/core"
)

// Answer decodes a color such as #2f6f5e, rgb(47 111 94), or
// hsl(164, 41%, 31%).
func Answer(query string) (core.Result, bool) {
	rgb, ok := parse(asciiLower(strings.TrimSpace(query)))
	if !ok {
		return core.Result{}, false
	}
	r, g, b := rgb[0], rgb[1], rgb[2]
	hex := fmt.Sprintf("#%02X%02X%02X", r, g, b)
	rgbText := fmt.Sprintf("rgb(%d, %d, %d)", r, g, b)
	h, s, l := toHSL(rgb)
	hslText := fmt.Sprintf("hsl(%d, %d%%, %d%%)", h, s, l)
	onWhite := (1 + 0.05) / (luminance(rgb) + 0.05)
	return core.Result{
		ID:       core.ID("color:" + hex),
		Kind:     core.KindColor,
		Title:    hex,
		Subtitle: fmt.Sprintf("%s · %s · %.1f:1 on white", rgbText, hslText, onWhite),
		Icon:     core.ColorIcon(hex),
		Actions: []core.ResultAction{
			core.Do("Copy HEX", core.Copy{Text: hex}),
			core.Do("Copy RGB", core.Copy{Text: rgbText}),
			core.Do("Copy HSL", core.Copy{Text: hslText}),
		},
	}, true
}

// parse reads lower-case text.
func parse(text string) ([3]uint8, bool) {
	if hex, ok := strings.CutPrefix(text, "#"); ok {
		return parseHex(hex)
	}
	if args, ok := functionArgs(text, "rgb"); ok {
		if len(args) != 3 {
			return [3]uint8{}, false
		}
		var rgb [3]uint8
		for i, arg := range args {
			n, err := strconv.ParseUint(strings.TrimPrefix(arg, "+"), 10, 8)
			if err != nil {
				return [3]uint8{}, false
			}
			rgb[i] = uint8(n)
		}
		return rgb, true
	}
	args, ok := functionArgs(text, "hsl")
	if !ok || len(args) != 3 {
		return [3]uint8{}, false
	}
	hue, ok := parseFloat(trimAllSuffix(args[0], "deg"))
	if !ok || math.IsInf(hue, 0) || math.IsNaN(hue) {
		return [3]uint8{}, false
	}
	s, okS := percent(args[1])
	l, okL := percent(args[2])
	if !okS || !okL {
		return [3]uint8{}, false
	}
	return fromHSL(hue, s, l), true
}

func parseHex(hex string) ([3]uint8, bool) {
	var digits []uint8
	for i := 0; i < len(hex); i++ {
		n, err := strconv.ParseUint(hex[i:i+1], 16, 8)
		if err != nil {
			return [3]uint8{}, false
		}
		digits = append(digits, uint8(n))
	}
	switch len(digits) {
	case 3:
		return [3]uint8{digits[0] * 17, digits[1] * 17, digits[2] * 17}, true
	case 6:
		return [3]uint8{digits[0]<<4 | digits[1], digits[2]<<4 | digits[3], digits[4]<<4 | digits[5]}, true
	}
	return [3]uint8{}, false
}

// functionArgs returns the arguments of `name(a, b, c)` or `name(a b c)`.
func functionArgs(text, name string) ([]string, bool) {
	inner, ok := strings.CutPrefix(text, name)
	if !ok {
		return nil, false
	}
	inner, ok = strings.CutPrefix(strings.TrimLeftFunc(inner, unicode.IsSpace), "(")
	if !ok {
		return nil, false
	}
	inner, ok = strings.CutSuffix(inner, ")")
	if !ok {
		return nil, false
	}
	return strings.FieldsFunc(inner, func(r rune) bool { return r == ',' || unicode.IsSpace(r) }), true
}

func asciiLower(s string) string {
	return strings.Map(func(r rune) rune {
		if 'A' <= r && r <= 'Z' {
			return r + 'a' - 'A'
		}
		return r
	}, s)
}

func trimAllSuffix(s, suffix string) string {
	for strings.HasSuffix(s, suffix) {
		s = strings.TrimSuffix(s, suffix)
	}
	return s
}

// parseFloat reads a decimal number. The Rust app took no hex floats, which
// strconv would.
func parseFloat(s string) (float64, bool) {
	if strings.ContainsAny(s, "xX_") {
		return 0, false
	}
	n, err := strconv.ParseFloat(s, 64)
	return n, err == nil
}

// percent reads `41%` as 0.41.
func percent(value string) (float64, bool) {
	number, ok := strings.CutSuffix(value, "%")
	if !ok {
		return 0, false
	}
	n, ok := parseFloat(number)
	if !ok || !(0 <= n && n <= 100) {
		return 0, false
	}
	return n / 100, true
}

func fromHSL(hue, saturation, lightness float64) [3]uint8 {
	hue = math.Mod(hue, 360)
	if hue < 0 {
		hue += 360
	}
	hue /= 60
	chroma := (1 - math.Abs(2*lightness-1)) * saturation
	x := chroma * (1 - math.Abs(math.Mod(hue, 2)-1))
	var r, g, b float64
	switch int(hue) {
	case 0:
		r, g = chroma, x
	case 1:
		r, g = x, chroma
	case 2:
		g, b = chroma, x
	case 3:
		g, b = x, chroma
	case 4:
		r, b = x, chroma
	default:
		r, b = chroma, x
	}
	base := lightness - chroma/2
	channel := func(v float64) uint8 {
		return uint8(min(max(math.Round((v+base)*255), 0), 255))
	}
	return [3]uint8{channel(r), channel(g), channel(b)}
}

// toHSL returns hue in degrees and saturation and lightness in percent, rounded.
func toHSL(rgb [3]uint8) (hue, saturation, lightness int) {
	r, g, b := float64(rgb[0])/255, float64(rgb[1])/255, float64(rgb[2])/255
	hi, lo := max(r, g, b), min(r, g, b)
	l := (hi + lo) / 2
	delta := hi - lo
	var h, s float64
	if delta != 0 {
		s = delta / (1 - math.Abs(2*l-1))
		switch hi {
		case r:
			h = math.Mod((g-b)/delta, 6)
			if h < 0 {
				h += 6
			}
		case g:
			h = (b-r)/delta + 2
		default:
			h = (r-g)/delta + 4
		}
		h *= 60
	}
	return int(math.Round(h)) % 360, int(math.Round(s * 100)), int(math.Round(l * 100))
}

// luminance is the WCAG relative luminance.
func luminance(rgb [3]uint8) float64 {
	linear := func(c uint8) float64 {
		v := float64(c) / 255
		if v <= 0.03928 {
			return v / 12.92
		}
		return math.Pow((v+0.055)/1.055, 2.4)
	}
	return 0.2126*linear(rgb[0]) + 0.7152*linear(rgb[1]) + 0.0722*linear(rgb[2])
}
