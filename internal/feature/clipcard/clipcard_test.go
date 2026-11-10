package clipcard

import (
	"reflect"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

var now = time.Date(2026, 10, 7, 12, 0, 0, 0, time.UTC)

func detect(t *testing.T, text string) Card {
	t.Helper()
	card, ok := Detect(text, now)
	if !ok {
		t.Fatalf("%q has no card", text)
	}
	return card
}

// copies are the labels and texts of a card's actions.
func copies(t *testing.T, card Card) []string {
	t.Helper()
	var out []string
	for _, a := range card.Actions {
		c, ok := a.Action.(core.Copy)
		if !ok {
			t.Fatalf("action %#v copies nothing", a.Action)
		}
		out = append(out, a.Label+": "+c.Text)
	}
	return out
}

// shown writes each line as the view draws it.
func shown(lines []Line) []string {
	var out []string
	for _, line := range lines {
		var b strings.Builder
		b.WriteString(strings.Repeat("  ", line.Indent))
		for _, t := range line.Tokens {
			b.WriteString(t.Text)
		}
		out = append(out, b.String())
	}
	return out
}

func TestReadsAColor(t *testing.T) {
	card := detect(t, " rgb(47 111 94)\n")
	if card.Color == nil || card.Time != nil || card.JSON != nil {
		t.Fatalf("card %+v", card)
	}
	if card.Color.Hex != "#2F6F5E" || card.Color.CopiedAs != "RGB" || card.Color.OnWhite.Grade != "AA" {
		t.Errorf("color %+v", *card.Color)
	}
	want := []string{"Copy HEX: #2F6F5E", "Copy RGB: rgb(47, 111, 94)", "Copy HSL: hsl(164, 41%, 31%)"}
	if got := copies(t, card); !reflect.DeepEqual(got, want) {
		t.Errorf("actions %q", got)
	}
}

func TestReadsAUnixTime(t *testing.T) {
	card := detect(t, "1791354301250")
	if card.Time == nil || card.Color != nil || card.JSON != nil {
		t.Fatalf("card %+v", card)
	}
	if !card.Time.Milliseconds || card.Time.Raw != "1791354301250" || card.Time.Relative != "6 hours ago" {
		t.Errorf("time %+v", *card.Time)
	}
	if got := copies(t, card); got[2] != "Copy ISO 8601: 2026-10-07T06:25:01.250Z" {
		t.Errorf("actions %q", got)
	}
}

func TestReadsJSONKeepingKeyOrderAndText(t *testing.T) {
	text := `{"id":"evt_8f","tags":["prod","canary"],"metrics":{"pods":6,"ok":true,"rollback":null},"empty":{},"n":1.50}`
	card := detect(t, text)
	j := card.JSON
	if j == nil || card.Color != nil || card.Time != nil {
		t.Fatalf("card %+v", card)
	}
	if j.Array || j.Count != 5 || j.Depth != 2 || j.MoreLines != 0 || j.Bytes != len(text) || j.Minified != text {
		t.Errorf("json %+v", *j)
	}
	wantKeys := []Key{{"id", ""}, {"tags", "[2]"}, {"metrics", "{3}"}, {"empty", "{0}"}, {"n", ""}}
	if !reflect.DeepEqual(j.Keys, wantKeys) {
		t.Errorf("keys %q", j.Keys)
	}
	lines := shown(j.Lines)
	for i, want := range map[int]string{
		0: "{", 1: `  "id": "evt_8f",`, 2: `  "tags": [`, 3: `    "prod",`, 5: "  ],",
		9: `    "rollback": null`, 11: `  "empty": {},`, 12: `  "n": 1.50`, 13: "}",
	} {
		if lines[i] != want {
			t.Errorf("line %d: %q, want %q", i, lines[i], want)
		}
	}
	if len(lines) != 14 {
		t.Errorf("%d lines: %q", len(lines), lines)
	}
	if j.Lines[1].Tokens[0].Kind != TokenKey || j.Lines[1].Tokens[2].Kind != TokenString ||
		j.Lines[9].Tokens[2].Kind != TokenLiteral || j.Lines[12].Tokens[2].Kind != TokenNumber {
		t.Errorf("token kinds %+v", j.Lines)
	}

	got := copies(t, card)
	pretty := strings.Join(lines, "\n")
	if want := []string{"Copy Pretty JSON: " + pretty, "Copy Minified JSON: " + text}; !reflect.DeepEqual(got, want) {
		t.Errorf("actions %q", got)
	}
	// Pretty JSON minifies back to the same text.
	if again := detect(t, pretty); copies(t, again)[1] != "Copy Minified JSON: "+text {
		t.Errorf("minified pretty JSON: %q", copies(t, again)[1])
	}
}

func TestCountsTheItemsOfAnArray(t *testing.T) {
	j := detect(t, `[1, [2, 3], {"a": []}]`).JSON
	if j == nil || !j.Array || j.Count != 3 || j.Depth != 3 || len(j.Keys) != 0 {
		t.Errorf("json %+v", j)
	}
	if j := detect(t, "[]").JSON; j == nil || j.Count != 0 || len(j.Lines) != 1 {
		t.Errorf("[]: %+v", j)
	}
}

func TestNamesAtMostTwelveKeys(t *testing.T) {
	var fields []string
	for i := range 14 {
		fields = append(fields, `"k`+strconv.Itoa(i)+`":{"a":1}`)
	}
	j := detect(t, "{"+strings.Join(fields, ",")+"}").JSON
	if len(j.Keys) != MaxJSONKeys || j.Count != 14 || j.Keys[11] != (Key{"k11", "{1}"}) {
		t.Errorf("keys %q, count %d", j.Keys, j.Count)
	}
}

func TestLongJSONShowsItsFirstLines(t *testing.T) {
	items := make([]string, 3000)
	for i := range items {
		items[i] = strconv.Itoa(i)
	}
	card := detect(t, "["+strings.Join(items, ",")+"]")
	j := card.JSON
	if len(j.Lines) != MaxJSONLines || j.MoreLines != 3002-MaxJSONLines {
		t.Errorf("%d lines, %d more", len(j.Lines), j.MoreLines)
	}
	if got := strings.Count(copies(t, card)[0], "\n") + 1; got != 3002 {
		t.Errorf("Copy Pretty JSON has %d lines", got)
	}
	if _, ok := Detect(strings.Repeat(" ", MaxCardBytes+1), now); ok {
		t.Error("text over the limit has a card")
	}
}

// Indents grow with depth, so deep JSON under the card limit would be
// gigabytes pretty: it gets no Copy Pretty JSON.
func TestDeepJSONHasNoPrettyCopy(t *testing.T) {
	deep := strings.Repeat("[", 3000) + strings.Repeat("]", 3000)
	items := make([]string, MaxCardBytes/(len(deep)+1))
	for i := range items {
		items[i] = deep
	}
	card := detect(t, "["+strings.Join(items, ",")+"]")
	if len(card.Actions) != 1 || card.Actions[0].Label != "Copy Minified JSON" {
		t.Errorf("actions %+v", card.Actions)
	}
	if len(card.JSON.Lines) != MaxJSONLines || card.JSON.Depth != 3001 {
		t.Errorf("%d lines, depth %d", len(card.JSON.Lines), card.JSON.Depth)
	}
}

func TestCutsALongTokenInTheViewOnly(t *testing.T) {
	long := strings.Repeat("é", MaxTokenChars+10)
	card := detect(t, `["`+long+`"]`)
	if got := card.JSON.Lines[1].Tokens[0].Text; got != `"`+long[:2*(MaxTokenChars-1)]+"…" {
		t.Errorf("token %q", got)
	}
	if !strings.Contains(copies(t, card)[0], long) {
		t.Error("Copy Pretty JSON cut the token")
	}
	if got := len([]rune(card.JSON.Minified)); got != MaxMinifiedChars {
		t.Errorf("minified preview of %d characters", got)
	}
}

func TestNoCardForOtherText(t *testing.T) {
	for _, text := range []string{
		"", "hello", "#12345", "rgb(300, 0, 0)", "999999999", "17913543x1",
		`{"a":`, "42", `"text"`, "[1,]", "{} trailing", "[\"\xff\"]",
	} {
		if card, ok := Detect(text, now); ok {
			t.Errorf("%q has a card: %+v", text, card)
		}
	}
}

func TestANewCopyGetsANewCardID(t *testing.T) {
	first := detect(t, "#fff")
	if first.ID != detect(t, "#fff").ID || first.ID == detect(t, "#ffff00").ID || first.ID == detect(t, " #fff").ID {
		t.Error("the ID does not follow the text")
	}
	if first.ID != "14b2857dd4720298" {
		t.Errorf("ID %s, want the FNV-1a hash", first.ID)
	}
}
