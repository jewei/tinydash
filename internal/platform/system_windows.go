package platform

import (
	"fmt"
	"os/exec"
	"runtime"
	"syscall"
	"unsafe"

	"github.com/jewei/tinydash/internal/core"
)

// plan is what one system command runs. Exactly one of program, native, and
// url is set.
type plan struct {
	program           string
	args              []string
	native            string // a Win32 call, by its name
	url               string // opened with the app's opener
	waitForKeyRelease bool
}

const (
	lockWorkStation    = "LockWorkStation"
	setSuspendState    = "SetSuspendState"
	emptyRecycleBin    = "SHEmptyRecycleBinW"
	monitorPowerOff    = "PostMessageW SC_MONITORPOWER"
	toggleColorMode    = "toggleDarkMode"
	openRecycleBin     = "explorer.exe shell:RecycleBinFolder"
	createNoWindow     = 0x0800_0000
	hwndBroadcast      = 0xffff
	wmSysCommand       = 0x0112
	wmSettingChange    = 0x001a
	scMonitorPower     = 0xf170
	smtoAbortIfHung    = 0x0002
	sherbNoConfirm     = 0x1
	sherbNoProgressUI  = 0x2
	sherbNoSound       = 0x4
	rrfRtRegDword      = 0x10
	regDword           = 4
	eUnexpected        = int32(-0x7fff0001) // 0x8000FFFF
	personalizationKey = `Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
)

var (
	user32   = syscall.NewLazyDLL("user32.dll")
	powrprof = syscall.NewLazyDLL("powrprof.dll")
	shell32  = syscall.NewLazyDLL("shell32.dll")
	advapi32 = syscall.NewLazyDLL("advapi32.dll")

	procLockWorkStation    = user32.NewProc("LockWorkStation")
	procPostMessage        = user32.NewProc("PostMessageW")
	procSendMessageTimeout = user32.NewProc("SendMessageTimeoutW")
	procSetSuspendState    = powrprof.NewProc("SetSuspendState")
	procSHEmptyRecycleBin  = shell32.NewProc("SHEmptyRecycleBinW")
	procRegGetValue        = advapi32.NewProc("RegGetValueW")
	procRegSetKeyValue     = advapi32.NewProc("RegSetKeyValueW")
)

func resolvePlan(cmd core.SystemCommand, env func(string) string, _ func(string) error) (plan, error) {
	return systemPlan(cmd, env)
}

// availableSystemCommands is nil: every OS command can run. The calls are
// Win32 or Windows' own shutdown, so a missing helper on PATH does not apply.
func availableSystemCommands(func(string) string, func(string) error) map[core.SystemCommand]bool {
	return nil
}

func systemPlan(cmd core.SystemCommand, _ func(string) string) (plan, error) {
	switch cmd {
	case core.CommandLock:
		return plan{native: lockWorkStation}, nil
	case core.CommandSleep:
		return plan{native: setSuspendState}, nil
	case core.CommandEmptyTrash:
		return plan{native: emptyRecycleBin}, nil
	case core.CommandRestart:
		return plan{program: "shutdown", args: []string{"/r", "/t", "0"}}, nil
	case core.CommandShutDown:
		return plan{program: "shutdown", args: []string{"/s", "/t", "0"}}, nil
	case core.CommandLogOut:
		return plan{program: "shutdown", args: []string{"/l"}}, nil
	case core.CommandOpenSystemSettings:
		return plan{url: "ms-settings:"}, nil
	case core.CommandSleepDisplays:
		return plan{native: monitorPowerOff, waitForKeyRelease: true}, nil
	case core.CommandToggleDarkMode:
		return plan{native: toggleColorMode}, nil
	case core.CommandOpenTrash:
		return plan{native: openRecycleBin}, nil
	}
	return plan{}, errUnsupported
}

func (p plan) execute(openURL func(string) error) error {
	// The Win32 calls take no pointers except the documented null window
	// handle and root path, meaning "no owner" and "all drives", and the
	// documented HWND_BROADCAST handle, meaning "every top-level window".
	switch p.native {
	case lockWorkStation:
		if ok, _, err := procLockWorkStation.Call(); ok == 0 {
			return fmt.Errorf("Windows could not lock the screen: %v", err)
		}
		return nil
	case setSuspendState:
		// BOOLEAN is one byte; the rest of the register is undefined.
		if ok, _, err := procSetSuspendState.Call(0, 0, 0); byte(ok) == 0 {
			return fmt.Errorf("Windows could not sleep: %v", err)
		}
		return nil
	case emptyRecycleBin:
		result, _, _ := procSHEmptyRecycleBin.Call(0, 0, sherbNoConfirm|sherbNoProgressUI|sherbNoSound)
		if hr := int32(result); !recycleBinEmptied(hr) {
			return fmt.Errorf("Windows could not empty the Recycle Bin: %v", syscall.Errno(uint32(hr)))
		}
		return nil
	case monitorPowerOff:
		// Posted, not sent: sending to every window could wait on a hung one.
		// 2 means "off" for SC_MONITORPOWER.
		if ok, _, err := procPostMessage.Call(hwndBroadcast, wmSysCommand, scMonitorPower, 2); ok == 0 {
			return fmt.Errorf("Windows could not turn off the displays: %v", err)
		}
		return nil
	case toggleColorMode:
		return toggleDarkMode()
	case openRecycleBin:
		// Explorer's exit code says nothing about success, so it is not awaited.
		c := exec.Command("explorer.exe", "shell:RecycleBinFolder")
		if err := c.Start(); err != nil {
			return fmt.Errorf("Could not open the Recycle Bin: %v", err)
		}
		go c.Wait()
		return nil
	}
	if p.url != "" {
		return openURL(p.url)
	}
	c := exec.Command(p.program, p.args...)
	c.SysProcAttr = &syscall.SysProcAttr{CreationFlags: createNoWindow}
	return run(c)
}

// recycleBinEmptied says whether an SHEmptyRecycleBinW result means the bin
// is now empty. An already empty bin reports E_UNEXPECTED; that is not a
// failure.
func recycleBinEmptied(result int32) bool {
	return result >= 0 || result == eUnexpected
}

// toggleDarkMode switches apps and the taskbar between light and dark, as
// Settings > Personalization > Colors does. A custom mix of the two becomes
// one mode, the opposite of the apps' mode.
func toggleDarkMode() error {
	key, err := syscall.UTF16PtrFromString(personalizationKey)
	if err != nil {
		return err
	}
	apps, _ := syscall.UTF16PtrFromString("AppsUseLightTheme")
	system, _ := syscall.UTF16PtrFromString("SystemUsesLightTheme")
	light, size := uint32(1), uint32(4)
	status, _, _ := procRegGetValue.Call(uintptr(syscall.HKEY_CURRENT_USER), uintptr(unsafe.Pointer(key)), uintptr(unsafe.Pointer(apps)), rrfRtRegDword, 0, uintptr(unsafe.Pointer(&light)), uintptr(unsafe.Pointer(&size)))
	next := nextLightTheme(uint32(status), light)
	for _, name := range []*uint16{apps, system} {
		status, _, _ := procRegSetKeyValue.Call(uintptr(syscall.HKEY_CURRENT_USER), uintptr(unsafe.Pointer(key)), uintptr(unsafe.Pointer(name)), regDword, uintptr(unsafe.Pointer(&next)), 4)
		if status != 0 {
			return fmt.Errorf("Windows could not change the color mode: %v", syscall.Errno(status))
		}
	}
	// Windows open now repaint in the new colors, as after Settings. Each
	// busy window may take the whole timeout, so the action does not wait.
	go func() {
		area, _ := syscall.UTF16PtrFromString("ImmersiveColorSet")
		procSendMessageTimeout.Call(hwndBroadcast, wmSettingChange, 0, uintptr(unsafe.Pointer(area)), smtoAbortIfHung, 1000, 0)
		runtime.KeepAlive(area)
	}()
	return nil
}

// nextLightTheme is the AppsUseLightTheme value that switches the mode,
// from the status and value of reading it: 1 (light) after dark, else 0
// (dark). No value yet means the default, light.
func nextLightTheme(readStatus, light uint32) uint32 {
	if readStatus == 0 && light == 0 {
		return 1
	}
	return 0
}
