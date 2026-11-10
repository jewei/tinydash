package core

import (
	"reflect"
	"testing"
)

// Stored IDs depend on these prefixes; changing one loses users' data.
func TestSourcePrefixesNeverChange(t *testing.T) {
	want := []string{"app", "file", "clip", "snippet", "link", "emoji", "system"}
	var got []string
	for _, s := range Sources() {
		got = append(got, s.Traits().Prefix)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("prefixes %q, want %q", got, want)
	}
}

func TestIDsRoundTrip(t *testing.T) {
	for _, s := range Sources() {
		id := s.ID("a:b")
		got, key, ok := ParseID(id)
		if !ok || got != s || key != "a:b" {
			t.Errorf("%q parsed as %v %q %v", id, got, key, ok)
		}
	}
	for _, id := range []ID{"", "app", "calc:12*8", ":x"} {
		if _, _, ok := ParseID(id); ok {
			t.Errorf("%q parsed as a source ID", id)
		}
	}
}

// The Rust app's traits, row by row.
func TestSourceTraits(t *testing.T) {
	if tr := SourceClip.Traits(); tr.LearnsFromUse || tr.Hideable || tr.Aliasable || tr.Suggestible {
		t.Errorf("clipboard entries are not ranked, hidden, aliased, or suggested: %+v", tr)
	}
	if tr := SourceSystem.Traits(); !tr.LearnsFromUse || tr.Suggestible {
		t.Errorf("system commands learn from use but are never suggested: %+v", tr)
	}
	if SourceLink.Traits().Category != CategorySnippets {
		t.Error("quicklinks live in the Snippets tab")
	}
}

// actions has one value of every action. A new action must be added here;
// the recorder below fails if one is missing.
var actions = []Action{
	Launch{}, Open{}, Reveal{}, OpenURL{}, Copy{}, CopySecret{}, CopyCalculation{},
	CopySavedCalculation{}, DeleteCalculation{}, ClearCalculations{}, CopyClip{},
	CopyClipText{}, DeleteClip{}, CopyPlainText{}, ClearClipboard{}, CopySnippet{},
	OpenQuicklink{}, System{}, Pin{}, Unpin{}, Hide{}, SetAlias{}, SetHotkey{},
	TurnOnCurrencyRates{}, Refresh{}, OpenSettings{}, CenterLauncher{}, TypeQuery{}, Quit{},
}

// The Rust list of actions that count as use (actions.rs).
var countsAsUse = map[string]bool{
	"launch": true, "open": true, "openUrl": true, "copy": true, "copyCalculation": true,
	"copySavedCalculation": true, "copySnippet": true, "openQuicklink": true, "system": true,
	"copyPlainText": true, "clearClipboard": true, "clearCalculations": true,
	"openSettings": true, "quit": true,
}

func TestEveryActionHasItsOwnHandlerMethodAndUseRule(t *testing.T) {
	seen := map[string]bool{}
	for _, a := range actions {
		var r recorder
		if err := a.Dispatch(&r); err != nil {
			t.Fatal(err)
		}
		if r.called != a.Name() {
			t.Errorf("%T dispatched to %q, want %q", a, r.called, a.Name())
		}
		if seen[a.Name()] {
			t.Errorf("two actions are named %q", a.Name())
		}
		seen[a.Name()] = true
		if a.CountsAsUse() != countsAsUse[a.Name()] {
			t.Errorf("%s counts as use: %v, Rust: %v", a.Name(), a.CountsAsUse(), countsAsUse[a.Name()])
		}
	}
	if n := reflect.TypeFor[Handler]().NumMethod(); n != len(actions) {
		t.Errorf("Handler has %d methods, the test lists %d actions", n, len(actions))
	}
}

// recorder notes which Handler method an action called, by its JSON name.
type recorder struct{ called string }

func (r *recorder) note(a Action) error                               { r.called = a.Name(); return nil }
func (r *recorder) Launch(a Launch) error                             { return r.note(a) }
func (r *recorder) Open(a Open) error                                 { return r.note(a) }
func (r *recorder) Reveal(a Reveal) error                             { return r.note(a) }
func (r *recorder) OpenURL(a OpenURL) error                           { return r.note(a) }
func (r *recorder) Copy(a Copy) error                                 { return r.note(a) }
func (r *recorder) CopySecret(a CopySecret) error                     { return r.note(a) }
func (r *recorder) CopyCalculation(a CopyCalculation) error           { return r.note(a) }
func (r *recorder) CopySavedCalculation(a CopySavedCalculation) error { return r.note(a) }
func (r *recorder) DeleteCalculation(a DeleteCalculation) error       { return r.note(a) }
func (r *recorder) ClearCalculations(a ClearCalculations) error       { return r.note(a) }
func (r *recorder) CopyClip(a CopyClip) error                         { return r.note(a) }
func (r *recorder) CopyClipText(a CopyClipText) error                 { return r.note(a) }
func (r *recorder) DeleteClip(a DeleteClip) error                     { return r.note(a) }
func (r *recorder) CopyPlainText(a CopyPlainText) error               { return r.note(a) }
func (r *recorder) ClearClipboard(a ClearClipboard) error             { return r.note(a) }
func (r *recorder) CopySnippet(a CopySnippet) error                   { return r.note(a) }
func (r *recorder) OpenQuicklink(a OpenQuicklink) error               { return r.note(a) }
func (r *recorder) System(a System) error                             { return r.note(a) }
func (r *recorder) Pin(a Pin) error                                   { return r.note(a) }
func (r *recorder) Unpin(a Unpin) error                               { return r.note(a) }
func (r *recorder) Hide(a Hide) error                                 { return r.note(a) }
func (r *recorder) SetAlias(a SetAlias) error                         { return r.note(a) }
func (r *recorder) SetHotkey(a SetHotkey) error                       { return r.note(a) }
func (r *recorder) TurnOnCurrencyRates(a TurnOnCurrencyRates) error   { return r.note(a) }
func (r *recorder) Refresh(a Refresh) error                           { return r.note(a) }
func (r *recorder) OpenSettings(a OpenSettings) error                 { return r.note(a) }
func (r *recorder) CenterLauncher(a CenterLauncher) error             { return r.note(a) }
func (r *recorder) TypeQuery(a TypeQuery) error                       { return r.note(a) }
func (r *recorder) Quit(a Quit) error                                 { return r.note(a) }
