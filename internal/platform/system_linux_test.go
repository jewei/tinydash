package platform

import (
	"errors"
	"reflect"
	"slices"
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
		{"X-Cinnamon", "x11", core.CommandOpenSystemSettings, &plan{program: "cinnamon-settings", launch: true}},
		{"MATE", "x11", core.CommandOpenSystemSettings, &plan{program: "mate-control-center", launch: true}},
		{"LXQt", "wayland", core.CommandOpenSystemSettings, &plan{program: "lxqt-config", launch: true}},
		{"Hyprland", "wayland", core.CommandOpenSystemSettings, nil},

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
		{"X-Cinnamon", "x11", core.CommandLogOut, &plan{program: "cinnamon-session-quit", args: []string{"--logout", "--no-prompt"}}},
		{"MATE", "x11", core.CommandLogOut, &plan{program: "mate-session-save", args: []string{"--logout-dialog"}}},
		{"LXQt", "wayland", core.CommandShutDown, &plan{program: "lxqt-leave", args: []string{"--shutdown"}}},
		{"Hyprland", "wayland", core.CommandLogOut, nil},
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
		{"x-cinnamon", core.CommandRestart, &plan{program: "cinnamon-session-quit", args: []string{"--reboot"}}},
		{"x-cinnamon", core.CommandShutDown, &plan{program: "cinnamon-session-quit", args: []string{"--power-off"}}},
		{"mate", core.CommandRestart, &plan{program: "mate-session-save", args: []string{"--shutdown-dialog"}}},
		{"mate", core.CommandShutDown, &plan{program: "mate-session-save", args: []string{"--shutdown-dialog"}}},
		{"lxqt", core.CommandRestart, &plan{program: "lxqt-leave", args: []string{"--reboot"}}},
		{"lxqt", core.CommandShutDown, &plan{program: "lxqt-leave", args: []string{"--shutdown"}}},
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
		{"hyprland", core.CommandRestart, &plan{program: "systemctl", args: []string{"reboot"}}},
		{"", core.CommandShutDown, &plan{program: "systemctl", args: []string{"poweroff"}}},
		{"hyprland", core.CommandLogOut, nil},
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
		{"x-cinnamon", plan{program: "cinnamon-session-quit", args: []string{"--logout", "--no-prompt"}}},
		{"mate", plan{program: "mate-session-save", args: []string{"--logout-dialog"}}},
		{"lxqt", plan{program: "lxqt-leave", args: []string{"--logout"}}},
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

// find is a PATH that contains only names.
func find(names ...string) func(string) error {
	return func(name string) error {
		if slices.Contains(names, name) {
			return nil
		}
		return errors.New(name + " not found")
	}
}

func commandSet(cmds ...core.SystemCommand) map[core.SystemCommand]bool {
	out := map[core.SystemCommand]bool{}
	for _, cmd := range cmds {
		out[cmd] = true
	}
	return out
}

func TestACommandIsOfferedOnlyWhenItCanRun(t *testing.T) {
	for _, tc := range []struct {
		desktop, session string
		found            []string
		want             []core.SystemCommand
	}{
		// Nothing installed: the map is empty, not nil, so no OS command is offered.
		{"XFCE", "x11", nil, nil},
		{"GNOME", "wayland", nil, nil},

		{"XFCE", "x11", []string{"loginctl"}, []core.SystemCommand{core.CommandLock}},
		{"", "", []string{"systemctl"}, []core.SystemCommand{core.CommandSleep, core.CommandRestart, core.CommandShutDown}},
		{"XFCE", "x11", []string{"gio"}, []core.SystemCommand{core.CommandEmptyTrash, core.CommandOpenTrash}},
		{"XFCE", "x11", []string{"xdg-open"}, []core.SystemCommand{core.CommandOpenTrash}},
		{"XFCE", "x11", []string{"xfce4-settings-manager"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"GNOME", "wayland", []string{"gnome-control-center"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"KDE", "wayland", []string{"systemsettings"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"KDE", "wayland", []string{"systemsettings5"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"X-Cinnamon", "x11", []string{"cinnamon-settings"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"MATE", "x11", []string{"mate-control-center"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"LXQt", "wayland", []string{"lxqt-config"}, []core.SystemCommand{core.CommandOpenSystemSettings}},
		{"Hyprland", "wayland", []string{"gnome-control-center"}, nil},

		{"XFCE", "x11", []string{"xset"}, []core.SystemCommand{core.CommandSleepDisplays}},
		{"KDE", "x11", []string{"xset"}, []core.SystemCommand{core.CommandSleepDisplays}},
		{"XFCE", "wayland", []string{"xset"}, nil},
		{"KDE", "wayland", []string{"kscreen-doctor"}, []core.SystemCommand{core.CommandSleepDisplays}},
		{"GNOME", "wayland", []string{"kscreen-doctor"}, nil},

		{"ubuntu:GNOME", "wayland", []string{"gsettings"}, []core.SystemCommand{core.CommandToggleDarkMode}},
		{"XFCE", "x11", []string{"gsettings"}, nil},
		{"KDE", "wayland", []string{"gsettings"}, nil},
		{"", "", []string{"gsettings"}, nil},

		{"XFCE", "x11", []string{"xfce4-session-logout"}, []core.SystemCommand{core.CommandLogOut, core.CommandRestart, core.CommandShutDown}},
		{"XFCE", "x11", []string{"systemctl"}, []core.SystemCommand{core.CommandSleep, core.CommandRestart, core.CommandShutDown}},
		{"GNOME", "wayland", []string{"gnome-session-quit"}, []core.SystemCommand{core.CommandLogOut, core.CommandRestart, core.CommandShutDown}},
		{"GNOME", "wayland", []string{"systemctl"}, []core.SystemCommand{core.CommandSleep, core.CommandRestart, core.CommandShutDown}},
		{"KDE", "wayland", []string{"dbus-send"}, []core.SystemCommand{core.CommandLogOut, core.CommandRestart, core.CommandShutDown}},
		{"KDE", "wayland", []string{"systemctl"}, []core.SystemCommand{core.CommandSleep, core.CommandRestart, core.CommandShutDown}},
		{"X-Cinnamon", "x11", []string{"cinnamon-session-quit"}, []core.SystemCommand{core.CommandLogOut, core.CommandRestart, core.CommandShutDown}},
		{"MATE", "x11", []string{"mate-session-save"}, []core.SystemCommand{core.CommandLogOut, core.CommandRestart, core.CommandShutDown}},
		{"LXQt", "wayland", []string{"lxqt-leave"}, []core.SystemCommand{core.CommandLogOut, core.CommandRestart, core.CommandShutDown}},
		{"Hyprland", "wayland", []string{"loginctl"}, []core.SystemCommand{core.CommandLock}},
	} {
		got := availableSystemCommands(environment(tc.desktop, tc.session), find(tc.found...))
		want := commandSet(tc.want...)
		if got == nil || !reflect.DeepEqual(got, want) {
			t.Errorf("on %s/%s with %v: got %v, want %v", tc.desktop, tc.session, tc.found, got, want)
		}
	}
}

func TestTheInstalledProgramIsTheOneThatRuns(t *testing.T) {
	for _, tc := range []struct {
		desktop, session string
		cmd              core.SystemCommand
		found            []string
		want             plan
	}{
		{"KDE", "wayland", core.CommandOpenSystemSettings, []string{"systemsettings", "systemsettings5"}, plan{program: "systemsettings", launch: true}},
		{"KDE", "wayland", core.CommandOpenSystemSettings, []string{"systemsettings5"}, plan{program: "systemsettings5", launch: true}},
		{"XFCE", "x11", core.CommandOpenTrash, []string{"gio", "xdg-open"}, plan{native: openTrash}},
		{"XFCE", "x11", core.CommandOpenTrash, []string{"xdg-open"}, plan{program: "xdg-open", args: []string{"trash:///"}, native: openTrash}},
		{"XFCE", "x11", core.CommandRestart, []string{"xfce4-session-logout", "systemctl"}, plan{program: "xfce4-session-logout", args: []string{"--reboot"}}},
		{"XFCE", "x11", core.CommandRestart, []string{"systemctl"}, plan{program: "systemctl", args: []string{"reboot"}}},
		{"XFCE", "x11", core.CommandShutDown, []string{"systemctl"}, plan{program: "systemctl", args: []string{"poweroff"}}},
		{"KDE", "wayland", core.CommandShutDown, []string{"systemctl"}, plan{program: "systemctl", args: []string{"poweroff"}}},
		{"KDE", "wayland", core.CommandShutDown, []string{"dbus-send", "systemctl"}, plan{kde: "logoutAndShutdown"}},
		{"GNOME", "x11", core.CommandEmptyTrash, []string{"gio"}, plan{program: "gio", args: []string{"trash", "--empty"}}},
		{"GNOME", "wayland", core.CommandToggleDarkMode, []string{"gsettings"}, plan{native: gnomeDarkMode}},
		{"GNOME", "x11", core.CommandSleepDisplays, []string{"xset"}, plan{program: "xset", args: []string{"dpms", "force", "off"}, waitForKeyRelease: true}},
	} {
		got, err := resolvePlan(tc.cmd, environment(tc.desktop, tc.session), find(tc.found...))
		if err != nil || !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s on %q with %v: plan %+v, error %v; want %+v", tc.cmd, tc.desktop, tc.found, got, err, tc.want)
		}
	}
}

func TestLogOutStaysHiddenWhenOnlySystemdIsInstalled(t *testing.T) {
	for _, desktop := range []string{"XFCE", "GNOME", "KDE", "X-Cinnamon", "MATE", "LXQt", "Hyprland", ""} {
		_, err := resolvePlan(core.CommandLogOut, environment(desktop, "x11"), find("systemctl"))
		if err != errUnsupported {
			t.Errorf("%s: error %v", desktop, err)
		}
	}
}

func TestAvailableSystemCommandsIsAFilter(t *testing.T) {
	// Nil would offer every command, including ones this machine cannot run.
	if AvailableSystemCommands() == nil {
		t.Fatal("nil")
	}
}

func TestEveryCommandIsOfferedOnGNOMEWhenItsProgramIsInstalled(t *testing.T) {
	found := []string{"loginctl", "systemctl", "gnome-session-quit", "gio", "gnome-control-center", "xset", "gsettings"}
	got := availableSystemCommands(environment("ubuntu:GNOME", "x11"), find(found...))
	for _, cmd := range core.SystemCommands() {
		if !got[cmd] {
			t.Errorf("%s is hidden", cmd)
		}
	}
}
