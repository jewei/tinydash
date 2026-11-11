package platform

import (
	"bytes"
	"errors"
	"fmt"
	"os/exec"
	"strings"

	"github.com/jewei/tinydash/internal/core"
)

// plan is what one system command runs. Exactly one of program, kde, and
// native is set, except Open Trash, which may name its opener in program.
type plan struct {
	program string
	args    []string
	// launch starts a GUI program and does not wait for it.
	launch            bool
	kde               string // a method of KDE's session manager
	native            string // openTrash or gnomeDarkMode
	waitForKeyRelease bool
}

const (
	openTrash     = "openTrash"
	gnomeDarkMode = "gnomeDarkMode"
)

// desktopKind is the session this machine is running, from the lowercase
// XDG_CURRENT_DESKTOP. An empty kind is a desktop TinyDash does not know.
func desktopKind(desktop string) string {
	desktop = strings.ToLower(desktop)
	switch {
	case strings.Contains(desktop, "gnome"):
		return "gnome"
	case strings.Contains(desktop, "kde"):
		return "kde"
	case strings.Contains(desktop, "xfce"):
		return "xfce"
	case strings.Contains(desktop, "cinnamon"):
		return "cinnamon"
	case strings.Contains(desktop, "mate"):
		return "mate"
	case strings.Contains(desktop, "lxqt"):
		return "lxqt"
	}
	return ""
}

func systemPlan(cmd core.SystemCommand, env func(string) string) (plan, error) {
	kind := desktopKind(env("XDG_CURRENT_DESKTOP"))
	switch cmd {
	case core.CommandLock:
		return plan{program: "loginctl", args: []string{"lock-session"}}, nil
	case core.CommandSleep:
		return plan{program: "systemctl", args: []string{"suspend"}}, nil
	case core.CommandLogOut, core.CommandRestart, core.CommandShutDown:
		if p, ok := sessionEnd(env("XDG_CURRENT_DESKTOP"), cmd); ok {
			return p, nil
		}
	case core.CommandEmptyTrash:
		return plan{program: "gio", args: []string{"trash", "--empty"}}, nil
	case core.CommandOpenSystemSettings:
		switch kind {
		case "gnome":
			return plan{program: "gnome-control-center", launch: true}, nil
		case "kde":
			return plan{program: "systemsettings", launch: true}, nil
		case "xfce":
			return plan{program: "xfce4-settings-manager", launch: true}, nil
		case "cinnamon":
			return plan{program: "cinnamon-settings", launch: true}, nil
		case "mate":
			return plan{program: "mate-control-center", launch: true}, nil
		case "lxqt":
			return plan{program: "lxqt-config", launch: true}, nil
		}
	case core.CommandOpenTrash:
		return plan{native: openTrash}, nil
	case core.CommandSleepDisplays:
		switch {
		case env("XDG_SESSION_TYPE") == "x11":
			return plan{program: "xset", args: []string{"dpms", "force", "off"}, waitForKeyRelease: true}, nil
		case kind == "kde":
			return plan{program: "kscreen-doctor", args: []string{"--dpms", "off"}, waitForKeyRelease: true}, nil
		}
	case core.CommandToggleDarkMode:
		if kind == "gnome" {
			return plan{native: gnomeDarkMode}, nil
		}
	}
	return plan{}, errUnsupported
}

// sessionEnd says how to log out, restart, or shut down on desktop, the
// XDG_CURRENT_DESKTOP value in any case. The session manager asks apps to quit
// first, so that they can offer to save, as macOS and Windows do. GNOME
// shows its own dialog to restart or shut down, as its system menu does: it
// has no way to skip it. Without a known session manager, systemd ends the
// session, which stops apps without asking, and Log Out is not supported.
func sessionEnd(desktop string, cmd core.SystemCommand) (plan, bool) {
	program := func(name string, args ...string) (plan, bool) {
		return plan{program: name, args: args}, true
	}
	switch desktopKind(desktop) {
	case "gnome":
		switch cmd {
		case core.CommandLogOut:
			return program("gnome-session-quit", "--logout", "--no-prompt")
		case core.CommandRestart:
			return program("gnome-session-quit", "--reboot")
		case core.CommandShutDown:
			return program("gnome-session-quit", "--power-off")
		}
	case "kde":
		switch cmd {
		case core.CommandLogOut:
			return plan{kde: "logout"}, true
		case core.CommandRestart:
			return plan{kde: "logoutAndReboot"}, true
		case core.CommandShutDown:
			return plan{kde: "logoutAndShutdown"}, true
		}
	case "xfce":
		switch cmd {
		case core.CommandLogOut:
			return program("xfce4-session-logout", "--logout")
		case core.CommandRestart:
			return program("xfce4-session-logout", "--reboot")
		case core.CommandShutDown:
			return program("xfce4-session-logout", "--halt")
		}
	case "cinnamon":
		switch cmd {
		case core.CommandLogOut:
			return program("cinnamon-session-quit", "--logout", "--no-prompt")
		case core.CommandRestart:
			return program("cinnamon-session-quit", "--reboot")
		case core.CommandShutDown:
			return program("cinnamon-session-quit", "--power-off")
		}
	case "mate":
		// MATE has a logout dialog and one shutdown dialog, which offers
		// both restart and shut down. The two power commands open that
		// dialog, and the user picks which.
		switch cmd {
		case core.CommandLogOut:
			return program("mate-session-save", "--logout-dialog")
		case core.CommandRestart, core.CommandShutDown:
			return program("mate-session-save", "--shutdown-dialog")
		}
	case "lxqt":
		switch cmd {
		case core.CommandLogOut:
			return program("lxqt-leave", "--logout")
		case core.CommandRestart:
			return program("lxqt-leave", "--reboot")
		case core.CommandShutDown:
			return program("lxqt-leave", "--shutdown")
		}
	}
	switch cmd {
	case core.CommandRestart:
		return program("systemctl", "reboot")
	case core.CommandShutDown:
		return program("systemctl", "poweroff")
	}
	return plan{}, false
}

// resolvePlan is the plan that can run on this machine. The desktop picks
// the program, and a missing program is skipped for one that does the same
// job, so a command is offered only when choosing it can work.
func resolvePlan(cmd core.SystemCommand, env func(string) string, lookPath func(string) error) (plan, error) {
	for _, p := range plansFor(cmd, env) {
		if p.ready(lookPath) {
			return p, nil
		}
	}
	return plan{}, errUnsupported
}

// plansFor lists how to run cmd, preferred first.
func plansFor(cmd core.SystemCommand, env func(string) string) []plan {
	p, err := systemPlan(cmd, env)
	if err != nil {
		return nil
	}
	plans := []plan{p}
	return append(plans, alternates(p, cmd)...)
}

// alternates do the same job when the preferred program is not installed.
// Restart and shut down fall back to systemd, which does not ask apps to
// quit. Log out has no such fallback, so it is left out.
func alternates(p plan, cmd core.SystemCommand) []plan {
	switch cmd {
	case core.CommandOpenSystemSettings:
		if p.program == "systemsettings" {
			// Plasma 5 called it systemsettings5. Plasma 6 dropped the suffix.
			return []plan{{program: "systemsettings5", launch: true}}
		}
	case core.CommandOpenTrash:
		return []plan{{program: "xdg-open", args: []string{"trash:///"}, native: openTrash}}
	case core.CommandRestart, core.CommandShutDown:
		if p.program != "systemctl" {
			arg := "reboot"
			if cmd == core.CommandShutDown {
				arg = "poweroff"
			}
			return []plan{{program: "systemctl", args: []string{arg}}}
		}
	}
	return nil
}

// availableSystemCommands is the OS commands resolvePlan can run. The map
// is never nil: an empty one offers no OS command.
func availableSystemCommands(env func(string) string, lookPath func(string) error) map[core.SystemCommand]bool {
	out := map[core.SystemCommand]bool{}
	for _, cmd := range core.SystemCommands() {
		if _, err := resolvePlan(cmd, env, lookPath); err == nil {
			out[cmd] = true
		}
	}
	return out
}

// helper is the program that must be installed for p to run.
func (p plan) helper() string {
	if p.program != "" {
		return p.program
	}
	switch {
	case p.kde != "":
		return "dbus-send"
	case p.native == openTrash:
		return "gio"
	case p.native == gnomeDarkMode:
		return "gsettings"
	}
	return ""
}

func (p plan) ready(lookPath func(string) error) bool {
	helper := p.helper()
	return helper != "" && lookPath(helper) == nil
}

func (p plan) execute(func(string) error) error {
	switch {
	case p.kde != "":
		return askKDESessionManager(p.kde)
	case p.native == openTrash:
		return showTrash(p)
	case p.native == gnomeDarkMode:
		return toggleGNOMEDarkMode()
	case p.launch:
		return launch(p.program)
	}
	return run(exec.Command(p.program, p.args...))
}

// launch starts a GUI program and returns at once; it keeps running on its
// own.
func launch(program string) error {
	c := exec.Command(program)
	if err := c.Start(); err != nil {
		return fmt.Errorf("Could not open %s: %v. Check that it is installed.", program, err)
	}
	// Reap it when it exits, so no zombie process stays behind.
	go c.Wait()
	return nil
}

// askKDESessionManager calls the method on the session bus and waits for
// the reply, so a failure reaches the user. dbus-send comes with D-Bus
// itself; KDE's own client is qdbus, qdbus6, or qdbus-qt6, depending on the
// distribution.
func askKDESessionManager(method string) error {
	c := exec.Command("dbus-send", "--session", "--print-reply", "--dest=org.kde.Shutdown", "/Shutdown", "org.kde.Shutdown."+method)
	var stderr bytes.Buffer
	c.Stderr = &stderr
	if err := c.Run(); err != nil {
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			if detail := strings.TrimSpace(stderr.String()); detail != "" {
				return fmt.Errorf("KDE could not end the session: %s", detail)
			}
		}
		return fmt.Errorf("KDE could not end the session: %v", err)
	}
	return nil
}

// showTrash shows the trash in the file manager. The output is discarded,
// not captured: a file manager that the opener starts directly inherits it,
// and a captured pipe would stay open until its window closes. gio and
// xdg-open themselves exit once the launch is done.
func showTrash(p plan) error {
	program, args := p.program, p.args
	if program == "" {
		program, args = "gio", []string{"open", "trash:///"}
	}
	err := exec.Command(program, args...).Run()
	if err == nil {
		return nil
	}
	var exit *exec.ExitError
	if !errors.As(err, &exit) {
		return fmt.Errorf("Could not run %s: %v", program, err)
	}
	return errors.New("No file manager could open the trash. Install one, such as Files.")
}

const gnomeInterface = "org.gnome.desktop.interface"

// toggleGNOMEDarkMode switches GNOME's color scheme between dark and the
// default, as its quick settings do.
func toggleGNOMEDarkMode() error {
	out, err := exec.Command("gsettings", "get", gnomeInterface, "color-scheme").Output()
	if err != nil {
		var exit *exec.ExitError
		if !errors.As(err, &exit) {
			return fmt.Errorf("Could not run gsettings: %v", err)
		}
		return errors.New("This desktop has no dark mode setting that TinyDash can change.")
	}
	return run(exec.Command("gsettings", "set", gnomeInterface, "color-scheme", nextColorScheme(string(out))))
}

// nextColorScheme is the color-scheme value that switches the mode, from
// the current value that gsettings printed.
func nextColorScheme(current string) string {
	if strings.Contains(current, "prefer-dark") {
		return "default"
	}
	return "prefer-dark"
}
