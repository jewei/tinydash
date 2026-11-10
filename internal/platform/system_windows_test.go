package platform

import (
	"reflect"
	"testing"

	"github.com/jewei/tinydash/internal/core"
)

func TestSystemPlanCoversEveryCommand(t *testing.T) {
	want := map[core.SystemCommand]plan{
		core.CommandLock:               {native: "LockWorkStation"},
		core.CommandSleep:              {native: "SetSuspendState"},
		core.CommandRestart:            {program: "shutdown", args: []string{"/r", "/t", "0"}},
		core.CommandShutDown:           {program: "shutdown", args: []string{"/s", "/t", "0"}},
		core.CommandLogOut:             {program: "shutdown", args: []string{"/l"}},
		core.CommandEmptyTrash:         {native: "SHEmptyRecycleBinW"},
		core.CommandOpenSystemSettings: {url: "ms-settings:"},
		core.CommandSleepDisplays:      {native: "PostMessageW SC_MONITORPOWER", waitForKeyRelease: true},
		core.CommandToggleDarkMode:     {native: "toggleDarkMode"},
		core.CommandOpenTrash:          {native: "explorer.exe shell:RecycleBinFolder"},
	}
	for _, cmd := range core.SystemCommands() {
		got, err := systemPlan(cmd, func(string) string { return "" })
		if err != nil {
			t.Errorf("%s: %v", cmd, err)
			continue
		}
		if !reflect.DeepEqual(got, want[cmd]) {
			t.Errorf("%s: plan %+v, want %+v", cmd, got, want[cmd])
		}
	}
}

func TestAnEmptyRecycleBinIsNotAFailure(t *testing.T) {
	for result, want := range map[uint32]bool{
		0:          true,
		1:          true,
		0x8000ffff: true,  // E_UNEXPECTED
		0x80070005: false, // E_ACCESSDENIED
	} {
		if got := recycleBinEmptied(int32(result)); got != want {
			t.Errorf("recycleBinEmptied(%#x) = %v, want %v", result, got, want)
		}
	}
}

func TestTogglesTheColorMode(t *testing.T) {
	for _, tc := range []struct{ status, light, want uint32 }{
		{0, 1, 0},
		{0, 0, 1},
		// ERROR_FILE_NOT_FOUND: no value yet means light, so dark comes next.
		{2, 0, 0},
	} {
		if got := nextLightTheme(tc.status, tc.light); got != tc.want {
			t.Errorf("nextLightTheme(%d, %d) = %d, want %d", tc.status, tc.light, got, tc.want)
		}
	}
}
