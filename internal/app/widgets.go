package app

import (
	"time"

	"github.com/jewei/tinydash/internal/feature/datetime"
	"github.com/jewei/tinydash/internal/platform"
	"github.com/jewei/tinydash/internal/settings"
	"github.com/jewei/tinydash/internal/ui/launcher"
)

// Widgets reads the widgets off the main thread, since the disk can make
// it wait, and hands them to the view on it.
func (b *backend) Widgets(done func(launcher.Widgets)) {
	a := b.app()
	s := a.engine.Snapshot(time.Now()).Settings
	go func() {
		w := loadWidgets(s, a.home, time.Now(), platform.DiskSpace)
		w.Focus = a.focus.widget()
		a.panel.Update(func() { done(w) })
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
// is left out; a disk that cannot be read says why in its card.
func loadWidgets(s settings.Settings, home string, now time.Time, diskSpace func(string) (platform.Volume, error)) launcher.Widgets {
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
	return w
}
