package platform

import (
	"bufio"
	"context"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/ebitengine/purego"
)

// FindFiles returns up to limit paths under folder whose names contain
// name, from the Spotlight index. It blocks up to a second, or until ctx
// ends, and returns the paths found by then.
func FindFiles(ctx context.Context, name, folder string, limit int) []string {
	// mdfind reads a name that starts with "-" as an unknown option, and it
	// takes no "--", so it gets the name without its leading dashes.
	asked := strings.TrimLeft(name, "-")
	if asked == "" || limit <= 0 {
		return nil
	}
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	command := exec.CommandContext(ctx, "/usr/bin/mdfind", "-onlyin", folder, "-name", asked)
	output, err := startOnTerminal(command)
	if err != nil {
		slog.Warn("Could not run Spotlight", "error", err)
		return nil
	}
	defer output.Close()
	// Without its dashes, the name also matches paths that the full name
	// does not. Drop them before the limit counts, or they can push out the
	// paths that match.
	keep := func(string) bool { return true }
	if asked != name {
		keep = func(path string) bool { return nameContains(path, name) }
	}
	paths := readPaths(ctx, output, limit, time.Second, keep)
	// Spotlight may still be writing after limit paths; stop it either way.
	cancel()
	_ = command.Wait()
	return paths
}

// nameContains reports whether the last part of path contains name,
// ignoring case.
func nameContains(path, name string) bool {
	return strings.Contains(strings.ToLower(filepath.Base(path)), strings.ToLower(name))
}

var openpty = sync.OnceValues(func() (func(master, slave *int32, name *byte, termios *syscall.Termios, size *byte) int32, error) {
	lib, err := purego.Dlopen("/usr/lib/libSystem.B.dylib", purego.RTLD_GLOBAL|purego.RTLD_NOW)
	if err != nil {
		return nil, err
	}
	var open func(master, slave *int32, name *byte, termios *syscall.Termios, size *byte) int32
	purego.RegisterLibFunc(&open, lib, "openpty")
	return open, nil
})

// startOnTerminal starts command with its output on a pseudo-terminal and
// returns the terminal's reading end. Into a pipe, mdfind writes only in
// 16 KB blocks or at its end, so a search that takes more than a second
// gave no paths; onto a terminal, it writes each path as it finds it.
func startOnTerminal(command *exec.Cmd) (*os.File, error) {
	open, err := openpty()
	if err != nil {
		return nil, err
	}
	// A raw terminal, as cfmakeraw makes one, keeps "\n" as it is: output
	// processing would turn it into "\r\n".
	raw := syscall.Termios{Cflag: syscall.CS8 | syscall.CREAD}
	raw.Cc[syscall.VMIN], raw.Cc[syscall.VTIME] = 1, 0
	var master, slave int32
	if open(&master, &slave, nil, &raw, nil) != 0 {
		return nil, os.NewSyscallError("openpty", syscall.EIO)
	}
	// Close-on-exec keeps both ends out of other children, which would
	// hold the terminal open after command ends.
	syscall.CloseOnExec(int(master))
	syscall.CloseOnExec(int(slave))
	reader := os.NewFile(uintptr(master), "pty")
	writer := os.NewFile(uintptr(slave), "tty")
	command.Stdout = writer
	err = command.Start()
	// The child holds its own copy of the writing end; closing this one
	// lets reads end with the child.
	writer.Close()
	if err != nil {
		reader.Close()
		return nil, err
	}
	return reader, nil
}

// readPaths returns up to limit paths, one per line, that output gives
// before timeout and keep accepts. The reader sends each path as it reads
// it, so a slow search still keeps the paths it found before the timeout.
func readPaths(ctx context.Context, output io.Reader, limit int, timeout time.Duration, keep func(string) bool) []string {
	found := make(chan string)
	stop := make(chan struct{})
	defer close(stop)
	go func() {
		defer close(found)
		lines := bufio.NewScanner(output)
		for lines.Scan() {
			if path := lines.Text(); path != "" && keep(path) {
				select {
				case found <- path:
				case <-stop:
					return
				}
			}
		}
	}()
	deadline := time.NewTimer(timeout)
	defer deadline.Stop()
	var paths []string
	for len(paths) < limit {
		select {
		case path, ok := <-found:
			if !ok {
				return paths
			}
			paths = append(paths, path)
		case <-deadline.C:
			return paths
		case <-ctx.Done():
			return paths
		}
	}
	return paths
}
