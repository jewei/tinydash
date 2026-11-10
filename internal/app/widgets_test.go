package app

import (
	"errors"
	"reflect"
	"testing"
	"time"

	"github.com/jewei/tinydash/internal/golden"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

func TestLoadsTheWidgetsThatAreOn(t *testing.T) {
	home := golden.Abs("/Users/me")
	now := time.Date(2026, 11, 9, 12, 0, 0, 0, time.UTC)
	volume := platform.Volume{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 182e9}
	var read []string
	disk := func(path string) (platform.Volume, error) {
		read = append(read, path)
		return volume, nil
	}
	notes := 0
	note := func() (string, error) {
		notes++
		return "Buy milk", nil
	}

	s := settings.Default()
	s.ClockCities = []string{"Tokyo", "Atlantis", "London"}
	s.ShowNotepad = true
	got := loadWidgets(s, home, now, disk, note, noClipboard(t))
	want := launcher.Widgets{
		Clocks: true,
		Cities: []launcher.City{
			{Name: "Tokyo", OffsetSeconds: 9 * 3600, Difference: "+9h"},
			{Name: "London", OffsetSeconds: 0, Difference: "Same time"},
		},
		Disk: &launcher.Disk{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 182e9},
		Note: &launcher.Note{Text: "Buy milk"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("widgets %+v, want %+v", got, want)
	}
	if len(read) != 1 || read[0] != home || notes != 1 {
		t.Errorf("read the disk of %q, want %q once; read the note %d times", read, home, notes)
	}

	s.ShowClocks, s.ShowDiskSpace, s.ShowNotepad = false, false, false
	read, notes = nil, 0
	if got := loadWidgets(s, home, now, disk, note, noClipboard(t)); !reflect.DeepEqual(got, launcher.Widgets{}) || len(read) != 0 || notes != 0 {
		t.Errorf("with every widget off: %+v, disk read %q, note read %d times", got, read, notes)
	}
}

// noClipboard is a clipboard that a test with clipboard cards off must
// never read.
func noClipboard(t *testing.T) func() (string, bool) {
	return func() (string, bool) {
		t.Error("read the clipboard with clipboard cards off")
		return "", false
	}
}

// The clipboard is read only for its cards, through a fake: never the
// user's clipboard.
func TestReadsTheClipboardOnlyForItsCards(t *testing.T) {
	s := settings.Default()
	s.ShowClocks, s.ShowDiskSpace, s.ShowClipboardCards = false, false, true
	now := time.Date(2026, 11, 9, 12, 0, 0, 0, time.UTC)
	text := func(t string, ok bool) func() (string, bool) { return func() (string, bool) { return t, ok } }

	got := loadWidgets(s, "", now, nil, nil, text("#2F6F5E", true))
	if got.Card == nil {
		t.Fatal("no card for a color")
	}
	want := &launcher.ClipCard{
		ID:    got.Card.ID,
		Color: &launcher.ClipColor{Hex: "#2F6F5E", OnWhite: launcher.Contrast{Ratio: "5.9:1", Grade: "AA"}, OnBlack: launcher.Contrast{Ratio: "3.6:1", Grade: "AA large"}},
		Rows:  []launcher.CardRow{{Label: "HEX", Value: "#2F6F5E"}, {Label: "RGB", Value: "rgb(47, 111, 94)"}, {Label: "HSL", Value: "hsl(164, 41%, 31%)"}},
	}
	want.Actions = got.Card.Actions
	if !got.ClipCards || !reflect.DeepEqual(got.Card, want) || got.Card.Actions[0].Label != "Copy HEX" {
		t.Errorf("color card %+v", got.Card)
	}

	unix := loadWidgets(s, "", now, nil, nil, text("1762689600", true)).Card
	if unix == nil || unix.Time == nil || unix.Rows[1] != (launcher.CardRow{Label: "UTC", Value: "Sun 9 Nov 2025, 12:00:00 UTC"}) || unix.Time.Relative != "1 year ago" {
		t.Errorf("time card %+v", unix)
	}

	object := loadWidgets(s, "", now, nil, nil, text(`{"a": [1, true]}`, true)).Card
	if object == nil || object.JSON == nil || len(object.JSON.Lines) != 6 || object.JSON.Keys[0] != (launcher.JSONKey{Name: "a", Hint: "[2]"}) ||
		object.JSON.Lines[2].Tokens[1].Kind != launcher.JSONPunctuation || object.JSON.Lines[3].Tokens[0].Kind != launcher.JSONLiteral {
		t.Errorf("json card %+v", object.JSON)
	}

	for _, read := range []func() (string, bool){text("hello", true), text("#fff", false)} {
		if got := loadWidgets(s, "", now, nil, nil, read); !got.ClipCards || got.Card != nil {
			t.Errorf("no card to show: %+v", got)
		}
	}
}

func TestShowsAtMostThreeCities(t *testing.T) {
	s := settings.Default()
	s.ClockCities = []string{"Tokyo", "London", "Paris", "Sydney"}
	s.ShowDiskSpace = false
	got := loadWidgets(s, "", time.Date(2026, 11, 9, 12, 0, 0, 0, time.UTC), nil, nil, noClipboard(t))
	if len(got.Cities) != 3 || got.Cities[2].Name != "Paris" {
		t.Errorf("cities %+v", got.Cities)
	}
}

func TestSaysWhyTheDiskCouldNotBeRead(t *testing.T) {
	s := settings.Default()
	s.ShowClocks = false
	failure := errors.New("The folder is gone.")
	got := loadWidgets(s, golden.Abs("/Users/me"), time.Now(), func(string) (platform.Volume, error) {
		return platform.Volume{}, failure
	}, nil, noClipboard(t))
	if got.Disk == nil || got.Disk.Err != failure {
		t.Errorf("disk %+v", got.Disk)
	}
}

// A note that cannot be read says why in its own card, and the other
// widgets still load.
func TestANoteThatCannotBeReadLeavesTheOtherWidgets(t *testing.T) {
	s := settings.Default()
	s.ShowNotepad = true
	failure := errors.New("Could not read or save data: no such table: note. Try again, or restart TinyDash.")
	got := loadWidgets(s, golden.Abs("/Users/me"), time.Date(2026, 11, 9, 12, 0, 0, 0, time.UTC),
		func(string) (platform.Volume, error) { return platform.Volume{Name: "Disk"}, nil },
		func() (string, error) { return "", failure }, noClipboard(t))
	if got.Note == nil || got.Note.Err != failure || !got.Clocks || got.Disk == nil {
		t.Errorf("widgets %+v, note %+v", got, got.Note)
	}
}

// The launcher gets each warning once, also one that came after it last
// asked.
func TestHandsEachWarningToTheLauncherOnce(t *testing.T) {
	a := &App{}
	a.warn("Could not register Control+Space.")
	a.warn(errors.New("The tray icon is unavailable."))
	a.warn("")
	a.warn(error(nil))
	b := (*backend)(a)
	if got := b.Warnings(); !reflect.DeepEqual(got, []string{"Could not register Control+Space.", "The tray icon is unavailable."}) {
		t.Errorf("warnings %q", got)
	}
	a.warn("Hotkey Control+Alt+S is off.")
	if got := b.Warnings(); !reflect.DeepEqual(got, []string{"Hotkey Control+Alt+S is off."}) {
		t.Errorf("warnings %q", got)
	}
	if got := b.Warnings(); got != nil {
		t.Errorf("warnings %q", got)
	}
}
