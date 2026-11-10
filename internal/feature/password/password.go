// Package password answers `password`, `pw 32`, `passphrase 8`, and `pin 4`
// with values from a random source. Copies are marked secret, so clipboard
// managers (TinyDash too) skip them.
package password

import (
	_ "embed"
	"encoding/binary"
	"fmt"
	"io"
	"math"
	"strconv"
	"strings"
	"sync"

	"github.com/jewei/tinydash/internal/core"
)

const (
	lower   = "abcdefghijklmnopqrstuvwxyz"
	upper   = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
	digits  = "0123456789"
	symbols = "!@#$%^&*-_=+?"
	all     = lower + upper + digits + symbols
)

//go:embed eff_large_wordlist.txt
var wordlist string

// words is the EFF large wordlist without its four hyphenated words (such as
// t-shirt): passphrases join words with hyphens, and the breaks must be clear.
var words = sync.OnceValue(func() []string {
	var out []string
	for line := range strings.Lines(wordlist) {
		_, word, ok := strings.Cut(strings.TrimRight(line, "\r\n"), "\t")
		if ok && !strings.Contains(word, "-") {
			out = append(out, word)
		}
	}
	return out
})

type kind struct {
	name                  string
	unit                  string
	fallback, least, most int // length: default, minimum, maximum
}

var (
	passwordKind   = kind{"Password", "characters", 20, 8, 64}
	passphraseKind = kind{"Passphrase", "words", 6, 3, 12}
	pinKind        = kind{"Pin", "digits", 6, 4, 12}
)

// Answers reads the query and draws the values from random. It returns an
// error only when random fails.
func Answers(query string, random io.Reader) ([]core.Result, error) {
	fields := strings.Fields(query)
	if len(fields) == 0 || len(fields) > 2 {
		return nil, nil
	}
	var kinds []kind
	switch strings.ToLower(fields[0]) {
	case "password", "pw":
		kinds = []kind{passwordKind, passphraseKind, pinKind}
	case "passphrase":
		kinds = []kind{passphraseKind}
	case "pin":
		kinds = []kind{pinKind}
	default:
		return nil, nil
	}
	length := -1
	if len(fields) == 2 {
		// Every kind's maximum fits in 16 bits; a larger number is out of range too.
		n, err := strconv.ParseUint(strings.TrimPrefix(fields[1], "+"), 10, 16)
		if err != nil {
			return nil, nil
		}
		// A length belongs to the first kind only: `password 24` is one password.
		length, kinds = int(n), kinds[:1]
	}
	var out []core.Result
	for _, k := range kinds {
		n := length
		if n < 0 {
			n = k.fallback
		}
		if n < k.least || n > k.most {
			continue
		}
		r, err := result(k, n, random)
		if err != nil {
			return nil, err
		}
		out = append(out, r)
	}
	return out, nil
}

func result(k kind, length int, random io.Reader) (core.Result, error) {
	var value string
	var bits float64
	var err error
	switch k {
	case passwordKind:
		value, err = password(length, random)
		bits = float64(length) * math.Log2(float64(len(all)))
	case passphraseKind:
		value, err = passphrase(length, random)
		bits = float64(length) * math.Log2(float64(len(words())))
	case pinKind:
		value, err = pick(digits, length, random)
		bits = float64(length) * math.Log2(10)
	}
	if err != nil {
		return core.Result{}, err
	}
	return core.Result{
		ID:       core.ID(fmt.Sprintf("password:%s:%d", k.name, length)),
		Kind:     core.KindPassword,
		Title:    value,
		Subtitle: fmt.Sprintf("%d %s · %d bits", length, k.unit, int(math.Floor(bits))),
		Icon:     core.SymbolIcon(core.SymbolKey),
		Actions:  []core.ResultAction{core.Do("Copy Password", core.CopySecret{Text: value})},
	}, nil
}

// password is uniform over the values of this length that have every
// character class.
func password(length int, random io.Reader) (string, error) {
	for {
		value, err := pick(all, length, random)
		if err != nil {
			return "", err
		}
		// Rejecting whole candidates keeps the result uniform over valid ones.
		if strings.ContainsAny(value, lower) && strings.ContainsAny(value, upper) &&
			strings.ContainsAny(value, digits) && strings.ContainsAny(value, symbols) {
			return value, nil
		}
	}
}

func passphrase(length int, random io.Reader) (string, error) {
	list := words()
	picked := make([]string, length)
	for i := range picked {
		at, err := below(len(list), random)
		if err != nil {
			return "", err
		}
		picked[i] = list[at]
	}
	return strings.Join(picked, "-"), nil
}

// pick draws length characters from an ASCII alphabet.
func pick(alphabet string, length int, random io.Reader) (string, error) {
	out := make([]byte, length)
	for i := range out {
		at, err := below(len(alphabet), random)
		if err != nil {
			return "", err
		}
		out[i] = alphabet[at]
	}
	return string(out), nil
}

// below is a uniform integer in [0, bound), without modulo bias.
func below(bound int, random io.Reader) (int, error) {
	zone := math.MaxUint32 - math.MaxUint32%uint32(bound)
	var buf [4]byte
	for {
		if _, err := io.ReadFull(random, buf[:]); err != nil {
			return 0, err
		}
		if v := binary.LittleEndian.Uint32(buf[:]); v < zone {
			return int(v % uint32(bound)), nil
		}
	}
}
