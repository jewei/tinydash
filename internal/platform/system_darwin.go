package platform

import (
	"bytes"
	"errors"
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/ebitengine/purego"
	"github.com/jewei/tinydash/internal/core"
)

// plan is what one system command runs. Exactly one of program, script,
// and native is set.
type plan struct {
	program           string
	args              []string
	script            string // AppleScript for osascript
	native            string // a C function of the login framework
	waitForKeyRelease bool
}

const lockScreen = "SACLockScreenImmediate"

func resolvePlan(cmd core.SystemCommand, env func(string) string, _ func(string) error) (plan, error) {
	return systemPlan(cmd, env)
}

// availableSystemCommands is nil: every OS command can run. The programs
// are part of macOS, so a missing helper on PATH does not apply.
func availableSystemCommands(func(string) string, func(string) error) map[core.SystemCommand]bool {
	return nil
}

func systemPlan(cmd core.SystemCommand, env func(string) string) (plan, error) {
	switch cmd {
	case core.CommandLock:
		return plan{native: lockScreen}, nil
	case core.CommandSleep:
		return plan{program: "/usr/bin/pmset", args: []string{"sleepnow"}}, nil
	case core.CommandRestart:
		return plan{script: `tell application "System Events" to restart`}, nil
	case core.CommandShutDown:
		return plan{script: `tell application "System Events" to shut down`}, nil
	case core.CommandLogOut:
		return plan{script: `tell application "System Events" to log out`}, nil
	case core.CommandEmptyTrash:
		return plan{script: "tell application \"Finder\"\nif (count of items of trash) > 0 then empty trash\nend tell"}, nil
	case core.CommandOpenSystemSettings:
		return plan{program: "/usr/bin/open", args: []string{"-b", "com.apple.systempreferences"}}, nil
	case core.CommandSleepDisplays:
		return plan{program: "/usr/bin/pmset", args: []string{"displaysleepnow"}, waitForKeyRelease: true}, nil
	case core.CommandToggleDarkMode:
		return plan{script: `tell application "System Events" to tell appearance preferences to set dark mode to not dark mode`}, nil
	case core.CommandOpenTrash:
		// Opening the folder needs no Automation permission, unlike asking Finder.
		home := env("HOME")
		if !filepath.IsAbs(home) {
			return plan{}, errors.New("Could not find your home folder.")
		}
		return plan{program: "/usr/bin/open", args: []string{filepath.Join(home, ".Trash")}}, nil
	}
	return plan{}, errUnsupported
}

func (p plan) execute(func(string) error) error {
	switch {
	case p.native == lockScreen:
		return lock()
	case p.script != "":
		return appleScript(p.script)
	}
	return run(exec.Command(p.program, p.args...))
}

func appleScript(script string) error {
	c := exec.Command("/usr/bin/osascript", "-e", script)
	var stderr bytes.Buffer
	c.Stderr = &stderr
	err := c.Run()
	if err == nil {
		return nil
	}
	var exit *exec.ExitError
	if !errors.As(err, &exit) {
		return fmt.Errorf("macOS could not run the command: %v. Try again.", err)
	}
	return appleScriptError(stderr.String())
}

// appleScriptError turns osascript's stderr into a message that says what
// the user can do.
func appleScriptError(stderr string) error {
	switch {
	case strings.Contains(stderr, "(-1743)"):
		return errors.New("macOS blocked the command. Allow TinyDash in System Settings > Privacy & Security > Automation.")
	case strings.Contains(stderr, "(-128)"):
		return errors.New("The command was canceled.")
	}
	return fmt.Errorf("macOS could not run the command: %s", strings.TrimSpace(stderr))
}

// lock locks with the login framework call that the system menu uses. It
// needs no Accessibility or Automation permission.
func lock() error {
	unavailable := errors.New("Lock Screen is unavailable. Press Control-Command-Q instead.")
	lib, err := purego.Dlopen("/System/Library/PrivateFrameworks/login.framework/Versions/Current/login", purego.RTLD_LAZY)
	if err != nil {
		return unavailable
	}
	sym, err := purego.Dlsym(lib, lockScreen)
	if err != nil {
		return unavailable
	}
	purego.SyscallN(sym)
	return nil
}
