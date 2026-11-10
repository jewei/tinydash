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

	s := settings.Default()
	s.ClockCities = []string{"Tokyo", "Atlantis", "London"}
	got := loadWidgets(s, home, now, disk)
	want := launcher.Widgets{
		Clocks: true,
		Cities: []launcher.City{
			{Name: "Tokyo", OffsetSeconds: 9 * 3600, Difference: "+9h"},
			{Name: "London", OffsetSeconds: 0, Difference: "Same time"},
		},
		Disk: &launcher.Disk{Name: "Macintosh HD", TotalBytes: 494e9, FreeBytes: 182e9},
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("widgets %+v, want %+v", got, want)
	}
	if len(read) != 1 || read[0] != home {
		t.Errorf("read the disk of %q, want %q once", read, home)
	}

	s.ShowClocks, s.ShowDiskSpace = false, false
	read = nil
	if got := loadWidgets(s, home, now, disk); !reflect.DeepEqual(got, launcher.Widgets{}) || len(read) != 0 {
		t.Errorf("with every widget off: %+v, disk read %q", got, read)
	}
}

func TestShowsAtMostThreeCities(t *testing.T) {
	s := settings.Default()
	s.ClockCities = []string{"Tokyo", "London", "Paris", "Sydney"}
	s.ShowDiskSpace = false
	got := loadWidgets(s, "", time.Date(2026, 11, 9, 12, 0, 0, 0, time.UTC), nil)
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
	})
	if got.Disk == nil || got.Disk.Err != failure {
		t.Errorf("disk %+v", got.Disk)
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
