package engine

import (
	"errors"
	"fmt"
	"runtime"
	"syscall"

	"github.com/jewei/tinydash/internal/settings"
)

// permissionStep is what to do when a settings save is refused. Only Windows
// refuses to replace a file that another app holds open.
func permissionStep(dir string) string {
	if runtime.GOOS == "windows" {
		return fmt.Sprintf(" Check that you can write to %s and that no other app has %s open.", dir, settings.FileName)
	}
	return fmt.Sprintf(" Check that you can write to %s.", dir)
}

// isWindowsDiskFull: Windows reports a full disk as ERROR_DISK_FULL (112) or
// ERROR_HANDLE_DISK_FULL (39), numbers that mean other errors elsewhere.
func isWindowsDiskFull(err error) bool {
	return runtime.GOOS == "windows" && (errors.Is(err, syscall.Errno(112)) || errors.Is(err, syscall.Errno(39)))
}
