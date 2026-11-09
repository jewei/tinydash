package engine

import (
	"errors"
	"fmt"
	"io"
	"io/fs"
	"log/slog"
	"os"
	"path/filepath"
	"syscall"

	"github.com/jewei/tinydash/internal/settings"
)

// SettingsFile is settings.json in the config folder. It refuses to save
// after a read failed for a reason other than its content, such as a lock
// held by a sync or antivirus tool: that file may still hold the user's
// settings, and this session has only defaults to write over it.
type SettingsFile struct {
	dir         string
	replaceable bool
}

// LoadSettings reads settings.json from dir. It never fails: a damaged file
// is moved aside and defaults are used, so a bad edit never stops the
// launcher. The warning, when not empty, is for the user.
func LoadSettings(dir string) (settings.Settings, string, SettingsFile) {
	path := filepath.Join(dir, settings.FileName)
	data, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		return settings.Default(), "", SettingsFile{dir, true}
	}
	if err != nil {
		return settings.Default(), settings.ReadFailedWarning(err), SettingsFile{dir, false}
	}
	loaded := settings.Load(data, true)
	backedUp := false
	if loaded.Backup {
		backup := filepath.Join(dir, settings.BackupName)
		if loaded.Damaged {
			backedUp = os.Rename(path, backup) == nil
		} else {
			backedUp = copyFile(path, backup) == nil
		}
	}
	return loaded.Settings, loaded.Warning(dir, backedUp), SettingsFile{dir, loaded.Replaceable(backedUp)}
}

// Save replaces settings.json with s. It writes a temporary file first, so a
// crash leaves either the old file or the new one.
func (f SettingsFile) Save(s settings.Settings) error {
	path := filepath.Join(f.dir, settings.FileName)
	if !f.replaceable {
		return fmt.Errorf("TinyDash could not use %s at startup, so it did not replace it. Quit and reopen TinyDash to change settings.", path)
	}
	text, err := s.Marshal()
	if err != nil {
		return err
	}
	temporary := path + ".tmp"
	err = os.MkdirAll(f.dir, 0o755)
	if err == nil {
		err = writeSynced(temporary, text)
	}
	if err == nil {
		err = os.Rename(temporary, path)
	}
	if err != nil {
		return saveFailed(f.dir, err)
	}
	return nil
}

func writeSynced(path string, data []byte) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	if _, err := file.Write(data); err != nil {
		file.Close()
		return err
	}
	// The data should be on disk before the rename: after a crash, a renamed
	// but empty file would load as defaults. Some volumes refuse the sync,
	// as SMB shares refuse the full sync of macOS; the save goes ahead there.
	if err := file.Sync(); err != nil {
		slog.Warn("Could not write the settings to disk before replacing them", "error", err)
	}
	return file.Close()
}

func copyFile(from, to string) error {
	in, err := os.Open(from)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.Create(to)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}

// saveFailed says why settings could not be saved. Here the folder is
// known, so a refused write or a full disk can say what to do; other causes
// have no step that fits them all.
func saveFailed(dir string, err error) error {
	step := ""
	switch {
	case errors.Is(err, syscall.ENOSPC), errors.Is(err, syscall.EDQUOT), isWindowsDiskFull(err):
		step = " Free some disk space, then try again."
	case errors.Is(err, fs.ErrPermission), errors.Is(err, syscall.EROFS):
		step = permissionStep(dir)
	}
	return fmt.Errorf("Could not save settings in %s: %v.%s", dir, err, step)
}
