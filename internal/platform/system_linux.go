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
// native is set.
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

func systemPlan(cmd core.SystemCommand, env func(string) string) (plan, error) {
	desktop := strings.ToLower(env("XDG_CURRENT_DESKTOP"))
	gnome := strings.Contains(desktop, "gnome")
	kde := strings.Contains(desktop, "kde")
	xfce := strings.Contains(desktop, "xfce")
	switch cmd {
	case core.CommandLock:
		return plan{program: "loginctl", args: []string{"lock-session"}}, nil
	case core.CommandSleep:
		return plan{program: "systemctl", args: []string{"suspend"}}, nil
	case core.CommandLogOut, core.CommandRestart, core.CommandShutDown:
		if p, ok := sessionEnd(desktop, cmd); ok {
			return p, nil
		}
	case core.CommandEmptyTrash:
		return plan{program: "gio", args: []string{"trash", "--empty"}}, nil
	case core.CommandOpenSystemSettings:
		switch {
		case gnome:
			return plan{program: "gnome-control-center", launch: true}, nil
		case kde:
			return plan{program: "systemsettings", launch: true}, nil
		case xfce:
			return plan{program: "xfce4-settings-manager", launch: true}, nil
		}
	case core.CommandOpenTrash:
		return plan{native: openTrash}, nil
	case core.CommandSleepDisplays:
		switch {
		case env("XDG_SESSION_TYPE") == "x11":
			return plan{program: "xset", args: []string{"dpms", "force", "off"}, waitForKeyRelease: true}, nil
		case kde:
			return plan{program: "kscreen-doctor", args: []string{"--dpms", "off"}, waitForKeyRelease: true}, nil
		}
	case core.CommandToggleDarkMode:
		if gnome {
			return plan{native: gnomeDarkMode}, nil
		}
	}
	return plan{}, errUnsupported
}

// sessionEnd says how to log out, restart, or shut down on desktop, the
// lowercase XDG_CURRENT_DESKTOP. The session manager asks apps to quit
// first, so that they can offer to save, as macOS and Windows do. GNOME
// shows its own dialog to restart or shut down, as its system menu does: it
// has no way to skip it. Without a known session manager, systemd ends the
// session, which stops apps without asking, and Log Out is not supported.
func sessionEnd(desktop string, cmd core.SystemCommand) (plan, bool) {
	gnome := strings.Contains(desktop, "gnome")
	kde := strings.Contains(desktop, "kde")
	xfce := strings.Contains(desktop, "xfce")
	program := func(name string, args ...string) (plan, bool) {
		return plan{program: name, args: args}, true
	}
	switch {
	case gnome:
		switch cmd {
		case core.CommandLogOut:
			return program("gnome-session-quit", "--logout", "--no-prompt")
		case core.CommandRestart:
			return program("gnome-session-quit", "--reboot")
		case core.CommandShutDown:
			return program("gnome-session-quit", "--power-off")
		}
	case kde:
		switch cmd {
		case core.CommandLogOut:
			return plan{kde: "logout"}, true
		case core.CommandRestart:
			return plan{kde: "logoutAndReboot"}, true
		case core.CommandShutDown:
			return plan{kde: "logoutAndShutdown"}, true
		}
	case xfce:
		switch cmd {
		case core.CommandLogOut:
			return program("xfce4-session-logout", "--logout")
		case core.CommandRestart:
			return program("xfce4-session-logout", "--reboot")
		case core.CommandShutDown:
			return program("xfce4-session-logout", "--halt")
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

func (p plan) execute(func(string) error) error {
	switch {
	case p.kde != "":
		return askKDESessionManager(p.kde)
	case p.native == openTrash:
		return showTrash()
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
// not captured: a file manager that GLib starts directly inherits it, and a
// captured pipe would stay open until its window closes. `gio open` itself
// exits once the launch is done.
func showTrash() error {
	err := exec.Command("gio", "open", "trash:///").Run()
	if err == nil {
		return nil
	}
	var exit *exec.ExitError
	if !errors.As(err, &exit) {
		return fmt.Errorf("Could not run gio: %v", err)
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
