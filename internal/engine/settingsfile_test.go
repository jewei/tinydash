package engine

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jewei/tinydash/internal/settings"
)

func write(t *testing.T, dir, text string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(dir, settings.FileName), []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
}

func exists(dir, name string) bool {
	_, err := os.Stat(filepath.Join(dir, name))
	return err == nil
}

func TestNoFileGivesDefaultsThatSave(t *testing.T) {
	dir := t.TempDir()
	s, warning, file := LoadSettings(dir)
	if warning != "" {
		t.Fatalf("warning %q", warning)
	}
	if err := file.Save(s); err != nil {
		t.Fatal(err)
	}
	again, warning, _ := LoadSettings(dir)
	if warning != "" || again.Shortcut != s.Shortcut {
		t.Fatalf("reloaded %+v, warning %q", again, warning)
	}
	if exists(dir, settings.FileName+".tmp") {
		t.Fatal("the temporary file stayed")
	}
}

// A damaged file, including one that is not UTF-8, is moved aside, and the
// defaults may then be saved.
func TestADamagedFileIsMovedAside(t *testing.T) {
	for _, text := range []string{`{"theme": `, "{\"theme\": \"\xff\"}"} {
		dir := t.TempDir()
		write(t, dir, text)
		_, warning, file := LoadSettings(dir)
		if !strings.Contains(warning, "Settings were invalid") || !strings.Contains(warning, settings.BackupName) {
			t.Errorf("%q: warning %q", text, warning)
		}
		if exists(dir, settings.FileName) || !exists(dir, settings.BackupName) {
			t.Errorf("%q: the file was not moved aside", text)
		}
		if err := file.Save(settings.Default()); err != nil {
			t.Errorf("%q: save after the move: %v", text, err)
		}
	}
}

// A value this version cannot read is copied aside before any save could
// write the default over it.
func TestAnUnreadableValueKeepsACopy(t *testing.T) {
	dir := t.TempDir()
	write(t, dir, `{"theme":"sage","hideOnBlur":false}`)
	s, warning, _ := LoadSettings(dir)
	if s.HideOnBlur || !strings.Contains(warning, "theme") {
		t.Fatalf("settings %+v, warning %q", s, warning)
	}
	if !exists(dir, settings.FileName) || !exists(dir, settings.BackupName) {
		t.Fatal("want the file kept and a copy beside it")
	}
}

// Keys of features this version dropped survive a save, so going back to
// an older TinyDash loses nothing.
func TestASaveKeepsKeysThisVersionDoesNotUse(t *testing.T) {
	dir := t.TempDir()
	write(t, dir, `{"futureSwitch":true,"futureCity":"Ipoh","futureFlag":1}`)
	s, _, file := LoadSettings(dir)
	if err := file.Save(s); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(filepath.Join(dir, settings.FileName))
	for _, key := range []string{`"futureSwitch": true`, `"futureCity": "Ipoh"`, `"futureFlag": 1`} {
		if !strings.Contains(string(data), key) {
			t.Errorf("saved file lost %s:\n%s", key, data)
		}
	}
}

// A file that exists but cannot be read may hold the user's settings, so
// it is never replaced.
func TestAFileThatCouldNotBeReadIsNeverReplaced(t *testing.T) {
	dir := t.TempDir()
	write(t, dir, `{"theme":"dark"}`)
	path := filepath.Join(dir, settings.FileName)
	if err := os.Chmod(path, 0); err != nil {
		t.Fatal(err)
	}
	defer os.Chmod(path, 0o644)
	if _, err := os.ReadFile(path); err == nil {
		t.Skip("this OS or user can read a file without read permission")
	}
	_, warning, file := LoadSettings(dir)
	if !strings.Contains(warning, "Could not read settings") {
		t.Fatalf("warning %q", warning)
	}
	if err := file.Save(settings.Default()); err == nil || !strings.Contains(err.Error(), "did not replace it") {
		t.Fatalf("save gave %v", err)
	}
}

func TestARefusedSaveSaysWhatToDo(t *testing.T) {
	dir := t.TempDir()
	_, _, file := LoadSettings(dir)
	if err := os.Chmod(dir, 0o500); err != nil {
		t.Fatal(err)
	}
	defer os.Chmod(dir, 0o755)
	if probe, err := os.Create(filepath.Join(dir, "probe")); err == nil {
		probe.Close()
		t.Skip("this OS or user can write to a folder without write permission")
	}
	err := file.Save(settings.Default())
	if err == nil || !strings.Contains(err.Error(), "Check that you can write to "+dir) {
		t.Fatalf("save gave %v", err)
	}
}
