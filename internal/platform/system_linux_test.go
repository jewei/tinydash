package platform

import (
	"reflect"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func environment(desktop, session string) func(string) string {
	return func(name string) string {
		switch name {
		case "XDG_CURRENT_DESKTOP":
			return desktop
		case "XDG_SESSION_TYPE":
			return session
		}
		return ""
	}
}

func TestSystemPlanOnEachDesktop(t *testing.T) {
	for _, tc := range []struct {
		desktop, session string
		cmd              core.SystemCommand
		want             *plan // nil: unsupported
	}{
		{"GNOME", "wayland", core.CommandLock, &plan{program: "loginctl", args: []string{"lock-session"}}},
		{"", "", core.CommandLock, &plan{program: "loginctl", args: []string{"lock-session"}}},
		{"KDE", "wayland", core.CommandSleep, &plan{program: "systemctl", args: []string{"suspend"}}},
		{"", "", core.CommandSleep, &plan{program: "systemctl", args: []string{"suspend"}}},
		{"XFCE", "x11", core.CommandEmptyTrash, &plan{program: "gio", args: []string{"trash", "--empty"}}},
		{"", "", core.CommandEmptyTrash, &plan{program: "gio", args: []string{"trash", "--empty"}}},
		{"", "", core.CommandOpenTrash, &plan{native: "openTrash"}},

		{"ubuntu:GNOME", "wayland", core.CommandOpenSystemSettings, &plan{program: "gnome-control-center", launch: true}},
		{"KDE", "wayland", core.CommandOpenSystemSettings, &plan{program: "systemsettings", launch: true}},
		{"XFCE", "x11", core.CommandOpenSystemSettings, &plan{program: "xfce4-settings-manager", launch: true}},
		{"X-Cinnamon", "x11", core.CommandOpenSystemSettings, nil},

		{"GNOME", "x11", core.CommandSleepDisplays, &plan{program: "xset", args: []string{"dpms", "force", "off"}, waitForKeyRelease: true}},
		{"KDE", "x11", core.CommandSleepDisplays, &plan{program: "xset", args: []string{"dpms", "force", "off"}, waitForKeyRelease: true}},
		{"KDE", "wayland", core.CommandSleepDisplays, &plan{program: "kscreen-doctor", args: []string{"--dpms", "off"}, waitForKeyRelease: true}},
		{"GNOME", "wayland", core.CommandSleepDisplays, nil},
		{"", "", core.CommandSleepDisplays, nil},

		{"ubuntu:GNOME", "wayland", core.CommandToggleDarkMode, &plan{native: "gnomeDarkMode"}},
		{"KDE", "wayland", core.CommandToggleDarkMode, nil},
		{"XFCE", "x11", core.CommandToggleDarkMode, nil},
		{"", "", core.CommandToggleDarkMode, nil},

		{"GNOME", "wayland", core.CommandLogOut, &plan{program: "gnome-session-quit", args: []string{"--logout", "--no-prompt"}}},
		{"KDE", "wayland", core.CommandRestart, &plan{kde: "logoutAndReboot"}},
		{"XFCE", "x11", core.CommandShutDown, &plan{program: "xfce4-session-logout", args: []string{"--halt"}}},
		{"X-Cinnamon", "x11", core.CommandLogOut, nil},
		{"", "", core.CommandRestart, &plan{program: "systemctl", args: []string{"reboot"}}},
	} {
		got, err := systemPlan(tc.cmd, environment(tc.desktop, tc.session))
		if tc.want == nil {
			if err != errUnsupported {
				t.Errorf("%s on %q/%q: plan %+v, error %v; want unsupported", tc.cmd, tc.desktop, tc.session, got, err)
			}
			continue
		}
		if err != nil || !reflect.DeepEqual(got, *tc.want) {
			t.Errorf("%s on %q/%q: plan %+v, error %v; want %+v", tc.cmd, tc.desktop, tc.session, got, err, *tc.want)
		}
	}
}

func TestUnsupportedErrorText(t *testing.T) {
	if got := errUnsupported.Error(); got != "This command is not supported on your desktop." {
		t.Fatal(got)
	}
}

func TestEveryCommandHasAPlanOnGNOMEWithX11(t *testing.T) {
	for _, cmd := range core.SystemCommands() {
		if _, err := systemPlan(cmd, environment("ubuntu:GNOME", "x11")); err != nil {
			t.Errorf("%s: %v", cmd, err)
		}
	}
}

func sessionPlan(desktop string, cmd core.SystemCommand) *plan {
	p, ok := sessionEnd(desktop, cmd)
	if !ok {
		return nil
	}
	return &p
}

func TestRestartAndShutDownGoThroughTheSessionManager(t *testing.T) {
	for _, tc := range []struct {
		desktop string
		cmd     core.SystemCommand
		want    *plan
	}{
		{"ubuntu:gnome", core.CommandRestart, &plan{program: "gnome-session-quit", args: []string{"--reboot"}}},
		{"ubuntu:gnome", core.CommandShutDown, &plan{program: "gnome-session-quit", args: []string{"--power-off"}}},
		{"kde", core.CommandRestart, &plan{kde: "logoutAndReboot"}},
		{"kde", core.CommandShutDown, &plan{kde: "logoutAndShutdown"}},
		{"xfce", core.CommandRestart, &plan{program: "xfce4-session-logout", args: []string{"--reboot"}}},
		{"xfce", core.CommandShutDown, &plan{program: "xfce4-session-logout", args: []string{"--halt"}}},
	} {
		if got := sessionPlan(tc.desktop, tc.cmd); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("sessionEnd(%q, %s) = %+v, want %+v", tc.desktop, tc.cmd, got, tc.want)
		}
	}
}

func TestOtherDesktopsRestartAndShutDownThroughSystemd(t *testing.T) {
	for _, tc := range []struct {
		desktop string
		cmd     core.SystemCommand
		want    *plan
	}{
		{"x-cinnamon", core.CommandRestart, &plan{program: "systemctl", args: []string{"reboot"}}},
		{"", core.CommandShutDown, &plan{program: "systemctl", args: []string{"poweroff"}}},
		{"x-cinnamon", core.CommandLogOut, nil},
		{"kde", core.CommandSleep, nil},
	} {
		if got := sessionPlan(tc.desktop, tc.cmd); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("sessionEnd(%q, %s) = %+v, want %+v", tc.desktop, tc.cmd, got, tc.want)
		}
	}
}

func TestLogOutGoesThroughTheSessionManager(t *testing.T) {
	for _, tc := range []struct {
		desktop string
		want    plan
	}{
		{"gnome", plan{program: "gnome-session-quit", args: []string{"--logout", "--no-prompt"}}},
		{"kde", plan{kde: "logout"}},
		{"xfce", plan{program: "xfce4-session-logout", args: []string{"--logout"}}},
	} {
		if got := sessionPlan(tc.desktop, core.CommandLogOut); got == nil || !reflect.DeepEqual(*got, tc.want) {
			t.Errorf("sessionEnd(%q, logOut) = %+v, want %+v", tc.desktop, got, tc.want)
		}
	}
}

func TestNextColorSchemeSwitchesTheMode(t *testing.T) {
	for current, want := range map[string]string{
		"'prefer-dark'\n":  "default",
		"'default'\n":      "prefer-dark",
		"'prefer-light'\n": "prefer-dark",
	} {
		if got := nextColorScheme(current); got != want {
			t.Errorf("nextColorScheme(%q) = %q, want %q", current, got, want)
		}
	}
}
