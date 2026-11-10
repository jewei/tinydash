package platform

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/ebitengine/purego/objc"
)

// withPool runs fn in an autorelease pool, on one thread.
func withPool(t *testing.T, fn func()) {
	t.Helper()
	if err := eventKit(); err != nil {
		t.Fatal(err)
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	pool := objc.ID(objc.GetClass("NSAutoreleasePool")).Send(objc.RegisterName("new"))
	defer pool.Send(objc.RegisterName("drain"))
	fn()
}

func TestReadsAndAsksForCalendarsOnlyAsItsOwnResponsibleProcess(t *testing.T) {
	itself := func(pid int32) int32 { return pid }
	terminal := func(int32) int32 { return 4242 }
	unknown := func(int32) int32 { return -1 }
	if !responsibleForItself(itself, 1234) {
		t.Error("TinyDash as its own responsible process")
	}
	for name, fn := range map[string]func(int32) int32{"terminal": terminal, "unknown": unknown, "missing": nil} {
		if responsibleForItself(fn, 1234) {
			t.Errorf("%s: reads as its own responsible process", name)
		}
	}
	// Started by another app, TinyDash does not even read the saved
	// answer, and so never asks either.
	withPool(t, func() {
		for _, kind := range []int{ekEntityEvent, ekEntityReminder} {
			if err := calendarAccess(kind, false); !errors.Is(err, ErrOpenedByAnotherApp) {
				t.Errorf("kind %d: %v", kind, err)
			}
		}
	})
}

func TestFindsTheProcessResponsibleForTinyDash(t *testing.T) {
	responsibleFor := findResponsibleFor()
	if responsibleFor == nil {
		t.Fatal("no responsibility_get_pid_responsible_for_pid")
	}
	// This process, or the app that started the tests.
	if pid := responsibleFor(int32(os.Getpid())); pid <= 0 {
		t.Errorf("responsible process %d", pid)
	}
}

// The packaged app's Info.plist comes from mygo.json, which must carry
// every usage string: macOS ends an app that asks without them.
func TestAsksForCalendarsOnlyWithEveryUsageString(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "mygo.json"))
	if err != nil {
		t.Fatal(err)
	}
	var config struct {
		MacOS struct {
			InfoPlist map[string]any `json:"infoPlist"`
		} `json:"macos"`
	}
	if err := json.Unmarshal(data, &config); err != nil {
		t.Fatal(err)
	}
	plist := func(keys []string) string {
		var b strings.Builder
		b.WriteString(`<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>`)
		for _, key := range keys {
			text, _ := config.MacOS.InfoPlist[key].(string)
			if text == "" {
				continue
			}
			b.WriteString("<key>" + key + "</key><string>" + text + "</string>")
		}
		b.WriteString("</dict></plist>")
		return b.String()
	}
	// NSBundle keeps a bundle per path, so each case has its own.
	folder := t.TempDir()
	for _, c := range []struct {
		name   string
		keys   []string
		canAsk bool
	}{
		{"Shipped.app", calendarUsageKeys, true},
		{"Partial.app", calendarUsageKeys[:3], false},
	} {
		contents := filepath.Join(folder, c.name, "Contents")
		if err := os.MkdirAll(contents, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(contents, "Info.plist"), []byte(plist(c.keys)), 0o644); err != nil {
			t.Fatal(err)
		}
		withPool(t, func() {
			bundle := objc.ID(objc.GetClass("NSBundle")).Send(objc.RegisterName("bundleWithPath:"), nsString(filepath.Join(folder, c.name)))
			if bundle == 0 {
				t.Fatalf("%s: no bundle", c.name)
			}
			if got := hasCalendarUsageStrings(bundle); got != c.canAsk {
				t.Errorf("%s: can ask %v", c.name, got)
			}
		})
	}
}

func TestReadsDueDatesInTheGregorianCalendar(t *testing.T) {
	withPool(t, func() {
		due := objc.ID(objc.GetClass("NSDateComponents")).Send(objc.RegisterName("new"))
		defer due.Send(objc.RegisterName("release"))
		for selector, value := range map[string]int{"setYear:": 2026, "setMonth:": 10, "setDay:": 30} {
			due.Send(objc.RegisterName(selector), value)
		}
		got, dayOnly, ok := dueDate(due)
		if !ok || !dayOnly || !got.Equal(time.Date(2026, 10, 30, 0, 0, 0, 0, time.UTC)) {
			t.Errorf("a day: %v %v %v", got, dayOnly, ok)
		}
		// Without a time zone of its own, a due time is local.
		due.Send(objc.RegisterName("setHour:"), 16)
		due.Send(objc.RegisterName("setMinute:"), 30)
		got, dayOnly, ok = dueDate(due)
		if !ok || dayOnly || !got.Equal(time.Date(2026, 10, 30, 16, 30, 0, 0, time.Local)) {
			t.Errorf("a time: %v %v %v", got, dayOnly, ok)
		}
		// A day that the components do not hold is no date.
		empty := objc.ID(objc.GetClass("NSDateComponents")).Send(objc.RegisterName("new"))
		defer empty.Send(objc.RegisterName("release"))
		if _, _, ok := dueDate(empty); ok {
			t.Error("components without a date read as one")
		}
	})
}

func TestReadsDatesToTheMillisecond(t *testing.T) {
	withPool(t, func() {
		at := time.Date(2026, 3, 10, 14, 0, 0, 123_000_000, time.UTC)
		if got := goTime(nsDate(at)); !got.Equal(at) {
			t.Errorf("%v, want %v", got, at)
		}
	})
}
