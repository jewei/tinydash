package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/feature/clipboard"
	"github.com/jewei/tinydash/internal/settings"
)

func TestUpdateSettingsSavesAndPublishes(t *testing.T) {
	e, _ := start(t)
	dir := t.TempDir()
	_, _, file := LoadSettings(dir)
	if err := <-e.UpdateSettings(file, func(s *settings.Settings) { s.HideOnBlur = false }); err != nil {
		t.Fatal(err)
	}
	if e.current.Load().Settings.HideOnBlur {
		t.Fatal("not published")
	}
	data, _ := os.ReadFile(filepath.Join(dir, settings.FileName))
	if !strings.Contains(string(data), `"hideOnBlur": false`) {
		t.Fatalf("not saved:\n%s", data)
	}
}

// Turning history off in Settings is a switch-off like any other: content
// read before it is never saved after it.
func TestTurningHistoryOffInSettingsDropsAStaleSave(t *testing.T) {
	e, _ := start(t)
	_, _, file := LoadSettings(t.TempDir())
	epoch := e.ClipEpoch()
	<-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = false })
	<-e.UpdateSettings(file, func(s *settings.Settings) { s.ClipboardHistoryEnabled = true })
	<-e.SaveClip(epoch, clipboard.Text("secret"), 100)
	if n := clips(t, e); n != 0 {
		t.Fatalf("%d clips from before history was off", n)
	}
}

func TestASettingsSaveThatFailsChangesNothing(t *testing.T) {
	e, changes := start(t)
	file := SettingsFile{dir: t.TempDir(), replaceable: false}
	if err := <-e.UpdateSettings(file, func(s *settings.Settings) { s.HideOnBlur = false }); err == nil {
		t.Fatal("a refused save passed")
	}
	if !e.current.Load().Settings.HideOnBlur || changes.Load() != 0 {
		t.Fatal("a failed save changed the published settings")
	}
}
