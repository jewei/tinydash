package launcher

import (
	"errors"
	"image/color"
	"testing"
	"time"

	"github.com/egoist/mygo/i18n"
	"github.com/egoist/mygo/ui"
	"github.com/jewei/tinydash/internal/core"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/kit"
)

// showLauncher starts a launcher over f and shows it, as the app does, which
// loads the widgets.
func showLauncher(t *testing.T, f *fake) (*Model, *ui.Tester) {
	t.Helper()
	m, tt := open(t, f)
	m.Show("")
	tt.Frame()
	return m, tt
}

func hasPane(tt *ui.Tester) bool {
	_, ok := tt.AccessByRole(ui.RoleRegion, "Widgets")
	return ok
}

func TestShowsWidgetsNextToAnEmptyAllSearch(t *testing.T) {
	f := newFake(func(q string, _ core.Category) []core.Result {
		if q != "" {
			return []core.Result{safari}
		}
		return nil
	})
	f.widgets = Widgets{Clocks: true, Cities: []City{{Name: "Tokyo", OffsetSeconds: 9 * 3600, Difference: "+8h"}}}
	_, tt := showLauncher(t, f)
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Clocks"); !ok || !hasPane(tt) {
		t.Fatalf("no clocks; texts %q", tt.Texts())
	}
	// The pane sits beside the empty view, which still shows its tips.
	if !tt.HasText("Tokyo") || !tt.HasText("+8h") || !tt.HasText("What do you need?") {
		t.Errorf("texts %q", tt.Texts())
	}

	tt.Type("saf")
	if hasPane(tt) || !tt.HasText("Safari") {
		t.Errorf("typing left the pane; texts %q", tt.Texts())
	}
	for range 3 {
		tt.Key(0, ui.KeyBackspace)
	}
	if !hasPane(tt) {
		t.Errorf("an empty search again shows no pane; texts %q", tt.Texts())
	}
	tt.Key(0, ui.KeyTab)
	if hasPane(tt) {
		t.Error("another tab shows the pane")
	}
	if f.widgetLoads != 1 {
		t.Errorf("loaded the widgets %d times, want once as the launcher showed", f.widgetLoads)
	}
}

func TestShowsNoPaneWithEveryWidgetOff(t *testing.T) {
	f := newFake(always())
	_, tt := showLauncher(t, f)
	if hasPane(tt) || !tt.HasText("What do you need?") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// The clocks move on as each minute turns, with no load in between: the
// view asks for its next frame.
func TestTheClocksTickEachMinute(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Clocks: true, Cities: []City{{Name: "UTC", Difference: "+0h"}}}
	_, tt := showLauncher(t, f)
	now := tt.Now()
	if !tt.HasText(now.Local().Format("15:04")) || !tt.HasText(now.UTC().Format("15:04")) {
		t.Fatalf("at %v: texts %q", now, tt.Texts())
	}
	next := now.Truncate(time.Minute).Add(time.Minute)
	tt.Advance(next.Sub(now))
	if !tt.HasText(next.Local().Format("15:04")) || !tt.HasText(next.UTC().Format("15:04")) {
		t.Errorf("at %v: texts %q", next, tt.Texts())
	}
	tt.Advance(time.Hour)
	later := next.Add(time.Hour)
	if !tt.HasText(later.Local().Format("15:04")) {
		t.Errorf("at %v: texts %q", later, tt.Texts())
	}
	if f.widgetLoads != 1 {
		t.Errorf("loaded the widgets %d times", f.widgetLoads)
	}
}

func TestShowsLocalTimeAloneWithoutCities(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Clocks: true}
	_, tt := showLauncher(t, f)
	now := tt.Now().Local()
	for _, text := range []string{now.Format("15:04"), now.Format("Monday"), now.Format("January 2, 2006")} {
		if !tt.HasText(text) {
			t.Errorf("no %q in %q", text, tt.Texts())
		}
	}
}

func TestShowsFreeDiskSpaceAndWarnsWhenLow(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Disk: &Disk{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 182e9}}
	m, tt := showLauncher(t, f)
	if !tt.HasText("Macintosh HD") || !tt.HasText("182 GB") || !tt.HasText("free of 494 GB") || tt.HasText("Low space") {
		t.Errorf("texts %q", tt.Texts())
	}
	if meter, ok := tt.AccessByRole(ui.RoleMeter, "Disk used"); !ok || meter.Now != 63 {
		t.Errorf("meter %+v", meter)
	}
	if _, ok := tt.AccessByRole(ui.RoleGroup, "Clocks"); ok {
		t.Error("the clocks show while they are off")
	}

	// Each show reads the disk again.
	f.widgets.Disk = &Disk{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 20e9}
	m.Show("")
	tt.Frame()
	if !tt.HasText("Low space") || !tt.HasText("20 GB") {
		t.Errorf("texts %q", tt.Texts())
	}

	f.widgets.Disk = &Disk{Err: errors.New("The folder is gone.")}
	m.Show("")
	tt.Frame()
	if !tt.HasText("Could not read the disk. The folder is gone.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// A change of the data, such as turning a widget off in Settings, loads
// the widgets again.
func TestLoadsTheWidgetsAgainWhenTheDataChanges(t *testing.T) {
	f := newFake(always())
	f.widgets = Widgets{Clocks: true}
	m, tt := showLauncher(t, f)
	f.widgets = Widgets{}
	m.Stale()
	tt.Frame()
	if hasPane(tt) {
		t.Error("the pane stays after every widget turned off")
	}
}

// A load that ends after a newer one does not put back older widgets.
func TestKeepsTheNewestLoad(t *testing.T) {
	f := newFake(always())
	f.holdWidgets = true
	m, tt := showLauncher(t, f)
	m.Stale()
	older, newer := f.loading[0], f.loading[1]
	newer(Widgets{Clocks: true})
	older(Widgets{})
	tt.Frame()
	if !hasPane(tt) {
		t.Error("an older load replaced the newer one")
	}
}

func TestDismissesAStartupWarningWithEscapeBeforeHiding(t *testing.T) {
	f := newFake(always())
	f.warnings = []string{"Could not register Control+Space.", "The tray icon is unavailable."}
	m, tt := showLauncher(t, f)
	if alert, ok := tt.AccessByRole(ui.RoleAlert, ""); !ok || !tt.HasText("Could not register Control+Space.") {
		t.Fatalf("alert %+v, texts %q", alert, tt.Texts())
	}
	tt.Key(0, ui.KeyEscape)
	if !tt.HasText("The tray icon is unavailable.") || f.hidden != 0 {
		t.Errorf("hidden %d, texts %q", f.hidden, tt.Texts())
	}
	pressButton(t, tt, "Dismiss")
	if _, ok := tt.AccessByRole(ui.RoleAlert, ""); ok || !tt.Focused("Search") {
		t.Errorf("after Dismiss: texts %q, search focused %v", tt.Texts(), tt.Focused("Search"))
	}
	tt.Key(0, ui.KeyEscape)
	if f.hidden != 1 {
		t.Errorf("hidden %d times", f.hidden)
	}

	// A warning that comes later shows the next time the launcher shows.
	f.warnings = []string{"Hotkey Control+Alt+S is off."}
	m.Show("")
	tt.Frame()
	if !tt.HasText("Hotkey Control+Alt+S is off.") {
		t.Errorf("texts %q", tt.Texts())
	}
}

// An action's failure shows in place of a warning, and Escape does not
// dismiss the warning behind it.
func TestAnActionFailureComesBeforeAWarning(t *testing.T) {
	f := newFake(always(safari))
	f.fail = func(core.Action) error { return errors.New("Safari is gone.") }
	f.warnings = []string{"Could not register Control+Space."}
	m, tt := showLauncher(t, f)
	tt.Key(0, ui.KeyEnter)
	if !tt.HasText("Safari is gone.") || tt.HasText("Could not register Control+Space.") {
		t.Fatalf("texts %q", tt.Texts())
	}
	if _, ok := tt.AccessByRole(ui.RoleButton, "Dismiss"); ok {
		t.Error("an action's failure has a Dismiss button")
	}
	tt.Key(0, ui.KeyEscape)
	if f.hidden != 1 || len(m.warnings) != 1 {
		t.Errorf("hidden %d, warnings %q", f.hidden, m.warnings)
	}
}

func TestFollowsTheAppearanceSetting(t *testing.T) {
	for _, c := range []struct {
		theme       settings.Theme
		desktopDark bool
		want        ui.Color
	}{
		{settings.ThemeSystem, false, kit.Light.Paper},
		{settings.ThemeSystem, true, kit.Dark.Paper},
		{settings.ThemeLight, true, kit.Light.Paper},
		{settings.ThemeDark, false, kit.Dark.Paper},
	} {
		f := newFake(always())
		f.settings.Theme = c.theme
		m := New(f)
		tt := ui.NewTesterWith(m.View, 760, 480, ui.TesterOptions{Dark: c.desktopDark})
		got := tt.Image().RGBAAt(700, 470)
		if got != rgba(c.want) {
			t.Errorf("%s on a dark desktop %v: background %v, want %v", c.theme, c.desktopDark, got, rgba(c.want))
		}
	}
}

func rgba(c ui.Color) color.RGBA { return color.RGBA{c.R, c.G, c.B, c.A} }

func TestFormatsBytesAsTheSolidAppDid(t *testing.T) {
	for bytes, want := range map[uint64]string{
		0:               "0 bytes",
		1:               "1 byte",
		999:             "999 bytes",
		1000:            "1.0 KB",
		9_960:           "10 KB",
		999_499:         "999 KB",
		999_500:         "1.0 MB",
		999_999:         "1.0 MB",
		182e9:           "182 GB",
		494e9:           "494 GB",
		2_500_000_000:   "2.5 GB",
		4_000e12:        "4000 TB",
		999_950_000_000: "1.0 TB",
	} {
		if got := formatBytes(bytes); got != want {
			t.Errorf("formatBytes(%d) = %q, want %q", bytes, got, want)
		}
	}
}

// The dates follow the user's regional formats, as the Rust app's Intl did;
// the time is 24-hour in every region, as the Rust app had it.
func TestTheClockDatesFollowTheRegion(t *testing.T) {
	for _, tc := range []struct {
		locale      string
		long, short string
	}{
		{"en-US", "January 2, 2006", "Mon, Jan 2"},
		{"en-GB", "2 January 2006", "Mon 2 Jan"},
	} {
		t.Run(tc.locale, func(t *testing.T) {
			f := newFake(always())
			f.widgets = Widgets{Clocks: true}
			m, tt := showLauncher(t, f)
			tt.SetRegion(i18n.Region{Locale: tc.locale, HourCycle: 12})
			now := tt.Now().Local()
			for _, text := range []string{now.Format("15:04"), now.Format("Monday"), now.Format(tc.long)} {
				if !tt.HasText(text) {
					t.Errorf("no %q in %q", text, tt.Texts())
				}
			}
			f.widgets.Cities = []City{{Name: "UTC", Difference: "+0h"}}
			m.Show("")
			tt.Frame()
			if text := now.Format(tc.short); !tt.HasText(text) {
				t.Errorf("no %q in %q", text, tt.Texts())
			}
		})
	}
}
