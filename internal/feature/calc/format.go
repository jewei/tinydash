package calc

import (
	"math/big"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"
)

// decimals is how many decimal places fend shows of a rounded value.
const decimals = 10

// String writes q as fend does: `approx. ` for a rounded value, `$` before
// the amount, and other units after it.
func (q quantity) String() string {
	number, exact := formatNumber(q.value, q.style, q.inexact)
	units := formatUnits(q.units, q.value.Cmp(big.NewRat(1, 1)) == 0)
	prefix := ""
	if !exact {
		prefix = "approx. "
	}
	switch units {
	case "$", "£", "¥":
		return prefix + units + number
	}
	return prefix + number + units
}

// formatNumber writes v and reports whether the text is exact. A whole
// number shows whole, a fraction that ends in decimals shows them all, and
// any other shows 10 decimal places, rounded half up.
func formatNumber(v *big.Rat, style style, inexact bool) (string, bool) {
	sign := ""
	if v.Sign() < 0 {
		sign = "-"
	}
	a := new(big.Rat).Abs(v)
	switch {
	case a.IsInt():
		return sign + a.Num().String(), !inexact
	case style == styleFraction:
		return sign + a.Num().String() + "/" + a.Denom().String(), !inexact
	case style == styleExact && !ends(a):
		whole, part := new(big.Int).QuoRem(a.Num(), a.Denom(), new(big.Int))
		fraction := part.String() + "/" + a.Denom().String()
		if whole.Sign() == 0 {
			return sign + fraction, !inexact
		}
		return sign + whole.String() + " " + fraction, !inexact
	}
	if places, ok := decimalPlaces(a); ok && !inexact {
		return sign + allDigits(a, places), true
	}
	scale := new(big.Int).Exp(big.NewInt(10), big.NewInt(decimals), nil)
	digits, rest := new(big.Int).QuoRem(new(big.Int).Mul(a.Num(), scale), a.Denom(), new(big.Int))
	if new(big.Int).Lsh(rest, 1).Cmp(a.Denom()) >= 0 {
		digits.Add(digits, big.NewInt(1))
	}
	if digits.Sign() == 0 {
		sign = ""
	}
	text := digits.String()
	if len(text) <= decimals {
		text = strings.Repeat("0", decimals+1-len(text)) + text
	}
	whole, fraction := text[:len(text)-decimals], strings.TrimRight(text[len(text)-decimals:], "0")
	if fraction != "" {
		whole += "." + fraction
	}
	return sign + whole, rest.Sign() == 0 && !inexact
}

// decimalPlaces is how many decimal places write a fraction in full, when it
// ends: when its denominator has no prime factors but 2 and 5.
func decimalPlaces(a *big.Rat) (int, bool) {
	d := new(big.Int).Set(a.Denom())
	twos := int(d.TrailingZeroBits())
	d.Rsh(d, uint(twos))
	fives := 0
	five, r := big.NewInt(5), new(big.Int)
	for {
		q, m := new(big.Int).QuoRem(d, five, r)
		if m.Sign() != 0 {
			break
		}
		d, fives = q, fives+1
	}
	return max(twos, fives), d.IsInt64() && d.Int64() == 1
}

func ends(a *big.Rat) bool {
	_, ok := decimalPlaces(a)
	return ok
}

func allDigits(a *big.Rat, places int) string {
	scale := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(places)), nil)
	text := new(big.Int).Quo(new(big.Int).Mul(a.Num(), scale), a.Denom()).String()
	if len(text) <= places {
		text = strings.Repeat("0", places+1-len(text)) + text
	}
	return text[:len(text)-places] + "." + text[len(text)-places:]
}

// formatUnits writes units as fend does: `m / s` for one unit below the
// line, `m^2`, and the last unit above the line plural unless the value
// is 1.
func formatUnits(units []component, one bool) string {
	var above, below []component
	for _, c := range units {
		if c.exp < 0 {
			below = append(below, c)
		} else {
			above = append(above, c)
		}
	}
	invert := len(above) > 0 && len(below) == 1
	var b strings.Builder
	for i, c := range append(above, below...) {
		if i > 0 || spaced(c.unit.singular) {
			b.WriteByte(' ')
		}
		exp := c.exp
		if invert && i >= len(above) {
			b.WriteString("/ ")
			exp = -exp
		}
		if !one && i == len(above)-1 {
			b.WriteString(c.unit.plural)
		} else {
			b.WriteString(c.unit.singular)
		}
		if exp != 1 {
			b.WriteString("^" + strconv.Itoa(exp))
		}
	}
	return b.String()
}

// spaced reports whether a space goes between the number and the unit: for
// a name such as `kg` or `°C`, but not for `%` or `$`.
func spaced(name string) bool {
	r, _ := utf8.DecodeRuneInString(name)
	return isAlphabetic(r) || r == '°'
}

// isAlphabetic is Rust's char::is_alphabetic.
func isAlphabetic(r rune) bool {
	return unicode.In(r, unicode.L, unicode.Nl, unicode.Other_Alphabetic)
}
