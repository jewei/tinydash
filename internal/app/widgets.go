package app

import (
	"time"

	"github.com/jewei/tinydash/internal/feature/clipcard"
	"github.com/jewei/tinydash/internal/feature/datetime"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// Widgets reads the widgets off the main thread, since the disk and the
// calendars can make it wait and the clipboard read waits for the main
// thread, and hands them to the view on it.
func (b *backend) Widgets(done func(launcher.Widgets)) {
	a := b.app()
	snapshot := a.engine.Snapshot(time.Now())
	s := snapshot.Settings
	go func() {
		now := time.Now()
		w := loadWidgets(s, a.home, now, platform.DiskSpace, a.engine.Note, a.desktop.clip.Text)
		// The zone the computer is in now, also after a change of it while
		// TinyDash runs: today ends at midnight there. It is read once, and
		// only for the widgets that use it.
		var zone *time.Location
		if s.ShowUpNext || s.ShowWeather {
			zone = platform.LocalZone()
		}
		if s.ShowUpNext {
			w.UpNext = loadUpNext(s, now.In(zone), platform.ReadUpNext)
		}
		if s.ShowWeather {
			w.Weather = weatherCard(s, snapshot.Weather, a.weather.failureFor(s.WeatherCity), zone)
		}
		a.panel.Update(func() { done(w) })
	}()
}

// SaveNote saves the Notepad's note off the main thread, which must not
// wait on the writer, not even to queue the save.
func (b *backend) SaveNote(text string, done func(error)) {
	a := b.app()
	go func() {
		err := <-a.engine.SaveNote(text)
		a.panel.Update(func() { done(err) })
	}()
}

// Warnings hands the warnings gathered so far to the launcher, once.
func (b *backend) Warnings() []string {
	a := b.app()
	a.mu.Lock()
	defer a.mu.Unlock()
	w := a.warnings
	a.warnings = nil
	return w
}

// loadWidgets gathers the widgets that are on. A city no time zone knows
// is left out; a disk or a note that cannot be read says why in its card.
// clipboardText reads the clipboard's text, as Clipboard.Text does, only
// while clipboard cards are on.
func loadWidgets(s settings.Settings, home string, now time.Time, diskSpace func(string) (platform.Volume, error), readNote func() (string, error), clipboardText func() (string, bool)) launcher.Widgets {
	var w launcher.Widgets
	if s.ShowClocks {
		w.Clocks = true
		for _, name := range s.ClockCities {
			if len(w.Cities) == datetime.MaxClockCities {
				break
			}
			if city, ok := datetime.City(name, now); ok {
				w.Cities = append(w.Cities, launcher.City{Name: city.Name, OffsetSeconds: city.OffsetSeconds, Difference: city.Difference})
			}
		}
	}
	if s.ShowDiskSpace {
		v, err := diskSpace(home)
		w.Disk = &launcher.Disk{Name: v.Name, TotalBytes: v.TotalBytes, FreeBytes: v.FreeBytes, Err: err}
	}
	if s.ShowNotepad {
		text, err := readNote()
		w.Note = &launcher.Note{Text: text, Err: err}
	}
	if s.ShowClipboardCards {
		w.ClipCards = true
		if text, ok := clipboardText(); ok {
			w.Card = clipCard(text, now)
		}
	}
	return w
}

// clipCard is the card of the clipboard text as the launcher draws it, or
// nil when the text shows none.
func clipCard(text string, now time.Time) *launcher.ClipCard {
	c, ok := clipcard.Detect(text, now)
	if !ok {
		return nil
	}
	out := &launcher.ClipCard{ID: c.ID, Actions: c.Actions}
	switch {
	case c.Color != nil:
		out.Color = &launcher.ClipColor{
			Hex: c.Color.Hex, CopiedAs: c.Color.CopiedAs,
			OnWhite: launcher.Contrast(c.Color.OnWhite), OnBlack: launcher.Contrast(c.Color.OnBlack),
		}
		out.Rows = []launcher.CardRow{{Label: "HEX", Value: c.Color.Hex}, {Label: "RGB", Value: c.Color.RGB}, {Label: "HSL", Value: c.Color.HSL}}
	case c.Time != nil:
		out.Time = &launcher.ClipTime{Raw: c.Time.Raw, Milliseconds: c.Time.Milliseconds, Relative: c.Time.Relative}
		out.Rows = []launcher.CardRow{{Label: "Local", Value: c.Time.Local}, {Label: "UTC", Value: c.Time.UTC}, {Label: "ISO", Value: c.Time.ISO}}
	case c.JSON != nil:
		out.JSON = clipJSON(c.JSON)
	}
	return out
}

func clipJSON(j *clipcard.JSON) *launcher.ClipJSON {
	out := &launcher.ClipJSON{
		Array: j.Array, Count: j.Count, Depth: j.Depth, Bytes: j.Bytes,
		Minified: j.Minified, MoreLines: j.MoreLines,
		Keys:  make([]launcher.JSONKey, len(j.Keys)),
		Lines: make([]launcher.JSONLine, len(j.Lines)),
	}
	for i, k := range j.Keys {
		out.Keys[i] = launcher.JSONKey(k)
	}
	for i, line := range j.Lines {
		tokens := make([]launcher.JSONToken, len(line.Tokens))
		for k, t := range line.Tokens {
			tokens[k] = launcher.JSONToken{Kind: tokenKinds[t.Kind], Text: t.Text}
		}
		out.Lines[i] = launcher.JSONLine{Indent: line.Indent, Tokens: tokens}
	}
	return out
}

var tokenKinds = map[clipcard.TokenKind]launcher.JSONTokenKind{
	clipcard.TokenKey: launcher.JSONKeyToken, clipcard.TokenString: launcher.JSONString,
	clipcard.TokenNumber: launcher.JSONNumber, clipcard.TokenLiteral: launcher.JSONLiteral,
	clipcard.TokenPunctuation: launcher.JSONPunctuation,
}
