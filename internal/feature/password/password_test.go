package password

import (
	"errors"
	"math/rand/v2"
	"slices"
	"strconv"
	"strings"
	"testing"
	"testing/iotest"

	"github.com/jewei/tinydash/internal/core"
)

func seeded() *rand.ChaCha8 { return rand.NewChaCha8([32]byte{1, 2, 3}) }

func copied(t *testing.T, r core.Result) string {
	t.Helper()
	c, ok := r.Actions[0].Action.(core.CopySecret)
	if !ok {
		t.Fatalf("unexpected action %#v", r.Actions[0].Action)
	}
	if c.Text != r.Title {
		t.Fatalf("copies %q, shows %q", c.Text, r.Title)
	}
	return c.Text
}

func answers(t *testing.T, query string) []core.Result {
	t.Helper()
	out, err := Answers(query, seeded())
	if err != nil {
		t.Fatal(err)
	}
	return out
}

func TestGeneratesEachKindWithinBounds(t *testing.T) {
	all := answers(t, "password")
	if len(all) != 3 {
		t.Fatalf("password gives %d answers", len(all))
	}
	if p := copied(t, all[0]); len(p) != 20 || !strings.ContainsAny(p, symbols) {
		t.Errorf("password %q", p)
	}
	if all[0].ID != "password:Password:20" || all[0].Subtitle != "20 characters · 124 bits" {
		t.Errorf("password %q %q", all[0].ID, all[0].Subtitle)
	}
	if p := copied(t, all[1]); strings.Count(p, "-") != 5 || all[1].Subtitle != "6 words · 77 bits" {
		t.Errorf("passphrase %q %q", p, all[1].Subtitle)
	}
	if p := copied(t, all[2]); len(p) != 6 || strings.Trim(p, digits) != "" || all[2].Subtitle != "6 digits · 19 bits" {
		t.Errorf("pin %q %q", p, all[2].Subtitle)
	}
	for query, want := range map[string]string{"pw 32": "password:Password:32", "PIN 4": "password:Pin:4", "passphrase 3": "password:Passphrase:3", "pw +8": "password:Password:8"} {
		got := answers(t, query)
		if len(got) != 1 || got[0].ID != core.ID(want) {
			t.Errorf("%q gives %v", query, got)
		}
	}
}

// Every value has the length asked for and only its kind's characters;
// a password has every class.
func TestValuesHaveTheirShape(t *testing.T) {
	random := seeded()
	list := words()
	for length := 8; length <= 64; length++ {
		r, err := Answers("pw "+strconv.Itoa(length), random)
		if err != nil {
			t.Fatal(err)
		}
		p := copied(t, r[0])
		if len(p) != length || strings.Trim(p, all) != "" {
			t.Errorf("password %q", p)
		}
		for _, class := range []string{lower, upper, digits, symbols} {
			if !strings.ContainsAny(p, class) {
				t.Errorf("password %q lacks one of %q", p, class)
			}
		}
	}
	for length := 3; length <= 12; length++ {
		r, _ := Answers("passphrase "+strconv.Itoa(length), random)
		got := strings.Split(copied(t, r[0]), "-")
		if len(got) != length {
			t.Errorf("passphrase %v has %d words", got, len(got))
		}
		for _, w := range got {
			if !slices.Contains(list, w) {
				t.Errorf("%q is not in the wordlist", w)
			}
		}
	}
	for length := 4; length <= 12; length++ {
		r, _ := Answers("pin "+strconv.Itoa(length), random)
		if p := copied(t, r[0]); len(p) != length || strings.Trim(p, digits) != "" {
			t.Errorf("pin %q", p)
		}
	}
}

func TestRejectsOtherQueriesAndOutOfRangeLengths(t *testing.T) {
	for _, q := range []string{"", "passwords", "password manager", "pin 2", "pin 13", "pw 999", "pw 7", "pw 65", "passphrase 2", "passphrase 13", "password 1 2", "pw -8", "pw 99999999999999999999999"} {
		if got := answers(t, q); len(got) != 0 {
			t.Errorf("%q gives %d answers", q, len(got))
		}
	}
}

func TestReportsAFailingRandomSource(t *testing.T) {
	broken := errors.New("no entropy")
	if _, err := Answers("pin", iotest.ErrReader(broken)); !errors.Is(err, broken) {
		t.Errorf("err %v", err)
	}
	if got, err := Answers("pinball", iotest.ErrReader(broken)); err != nil || got != nil {
		t.Errorf("no answer must read nothing: %v %v", got, err)
	}
}

func TestTheWordlistIsCompleteWithoutHyphenatedWords(t *testing.T) {
	list := words()
	if len(list) != 7772 || list[0] != "abacus" {
		t.Errorf("%d words, first %q", len(list), list[0])
	}
	for _, w := range list {
		if strings.Contains(w, "-") || w == "" {
			t.Errorf("word %q", w)
		}
	}
}

func TestBelowIsUniformWithoutBias(t *testing.T) {
	random := seeded()
	counts := make([]int, 13)
	const draws = 13_000
	for range draws {
		n, err := below(13, random)
		if err != nil {
			t.Fatal(err)
		}
		counts[n]++
	}
	for i, c := range counts {
		if c < 850 || c > 1150 {
			t.Errorf("%d drawn %d times of %d", i, c, draws)
		}
	}
}
