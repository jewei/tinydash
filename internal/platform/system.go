package platform

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"

	"github.com/jewei/tinydash/internal/core"
)

// RunSystemCommand runs an OS command from the System tab. openURL opens a
// URL with the app's opener; Windows opens its Settings app that way.
func RunSystemCommand(cmd core.SystemCommand, openURL func(string) error) error {
	p, err := resolvePlan(cmd, os.Getenv, lookup)
	if err != nil {
		return err
	}
	if p.waitForKeyRelease {
		waitForKeyRelease()
	}
	return p.execute(openURL)
}

var errUnsupported = errors.New("This command is not supported on your desktop.")

// AvailableSystemCommands reports the OS commands this machine can run.
// Nil means every command, which is macOS and Windows. On Linux a command
// is left out when this desktop cannot run it or its program is not
// installed, so search does not offer a command that would fail.
func AvailableSystemCommands() map[core.SystemCommand]bool {
	return availableSystemCommands(os.Getenv, lookup)
}

// lookup reports whether name is an executable on PATH. exec.LookPath also
// returns the path, which the plans do not need.
func lookup(name string) error {
	_, err := exec.LookPath(name)
	return err
}

// waitForKeyRelease waits until the user lets go of the key that ran Sleep
// Displays, which would wake the displays again.
func waitForKeyRelease() {
	time.Sleep(time.Second)
}

// run runs a helper program with fixed arguments and waits for it. Never
// pass user text here.
func run(c *exec.Cmd) error {
	program := c.Args[0]
	var stderr bytes.Buffer
	c.Stderr = &stderr
	err := c.Run()
	if err == nil {
		return nil
	}
	var exit *exec.ExitError
	if !errors.As(err, &exit) {
		return fmt.Errorf("Could not run %s: %v. Check that it is installed.", program, err)
	}
	return runFailure(program, stderr.String())
}

// runFailure is the error for a program that ended with an error, with the
// reason it wrote to stderr.
func runFailure(program, stderr string) error {
	detail := strings.TrimRight(strings.TrimSpace(stderr), ".")
	if detail == "" {
		return fmt.Errorf("%s failed. Try again.", program)
	}
	return fmt.Errorf("%s failed: %s. Try again.", program, detail)
}
