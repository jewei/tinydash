package platform

import (
	"errors"
	"reflect"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func TestSystemPlanCoversEveryCommand(t *testing.T) {
	home := func(name string) string {
		if name == "HOME" {
			return "/Users/me"
		}
		return ""
	}
	want := map[core.SystemCommand]plan{
		core.CommandLock:               {native: "SACLockScreenImmediate"},
		core.CommandSleep:              {program: "/usr/bin/pmset", args: []string{"sleepnow"}},
		core.CommandRestart:            {script: `tell application "System Events" to restart`},
		core.CommandShutDown:           {script: `tell application "System Events" to shut down`},
		core.CommandLogOut:             {script: `tell application "System Events" to log out`},
		core.CommandEmptyTrash:         {script: "tell application \"Finder\"\nif (count of items of trash) > 0 then empty trash\nend tell"},
		core.CommandOpenSystemSettings: {program: "/usr/bin/open", args: []string{"-b", "com.apple.systempreferences"}},
		core.CommandSleepDisplays:      {program: "/usr/bin/pmset", args: []string{"displaysleepnow"}, waitForKeyRelease: true},
		core.CommandToggleDarkMode:     {script: `tell application "System Events" to tell appearance preferences to set dark mode to not dark mode`},
		core.CommandOpenTrash:          {program: "/usr/bin/open", args: []string{"/Users/me/.Trash"}},
	}
	for _, cmd := range core.SystemCommands() {
		got, err := systemPlan(cmd, home)
		if err != nil {
			t.Errorf("%s: %v", cmd, err)
			continue
		}
		if !reflect.DeepEqual(got, want[cmd]) {
			t.Errorf("%s: plan %+v, want %+v", cmd, got, want[cmd])
		}
	}
}

func TestOpenTrashNeedsAHomeFolder(t *testing.T) {
	for _, home := range []string{"", "relative"} {
		_, err := systemPlan(core.CommandOpenTrash, func(string) string { return home })
		if err == nil || err.Error() != "Could not find your home folder." {
			t.Errorf("HOME=%q: error %v", home, err)
		}
	}
}

func TestEveryCommandStaysAvailable(t *testing.T) {
	missing := func(string) error { return errors.New("not found") }
	if got := availableSystemCommands(func(string) string { return "" }, missing); got != nil {
		t.Fatalf("available %v, want every command", got)
	}
}

func TestUnknownCommandIsUnsupported(t *testing.T) {
	if _, err := systemPlan("reboot", func(string) string { return "" }); err != errUnsupported {
		t.Fatalf("error %v, want %v", err, errUnsupported)
	}
}

func TestAppleScriptErrorSaysWhatToDo(t *testing.T) {
	for _, tc := range []struct{ stderr, want string }{
		{"execution error: Not authorized to send Apple events to System Events. (-1743)\n", "macOS blocked the command. Allow TinyDash in System Settings > Privacy & Security > Automation."},
		{"execution error: User canceled. (-128)\n", "The command was canceled."},
		{"  execution error: Finder got an error. (-1708)\n", "macOS could not run the command: execution error: Finder got an error. (-1708)"},
	} {
		if got := appleScriptError(tc.stderr).Error(); got != tc.want {
			t.Errorf("appleScriptError(%q) = %q, want %q", tc.stderr, got, tc.want)
		}
	}
}
