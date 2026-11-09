package app

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"log/slog"
	"slices"

	"github.com/egoist/mygo"

	"github.com/jewei/tinydash/internal/core"
)

// Usage is the help text of the command line. The Rust app took the same
// options, and its login item still starts TinyDash with --background.
const Usage = `Usage: tinydash [--settings | --background | --mode <category>] [--data-dir <folder>]

  (no option)          Show the launcher, or toggle it if TinyDash is running
  --settings           Open Settings
  --background         Start without showing a window (used at login)
  --hidden             The same as --background
  --mode <category>    Show the launcher in a category with an empty query:
                       all, apps, files, clipboard, snippets, emoji, system
  --data-dir <folder>  Keep settings.json and tinydash.db in this folder
  --help               Print this help`

// Launch is what a command line asks of TinyDash. A second start hands its
// command line to the running TinyDash, so it also drives that one.
type Launch struct {
	// Settings opens Settings.
	Settings bool
	// Background shows no window, as at login.
	Background bool
	// Category shows the launcher on this tab, or on All for "".
	Category core.Category
}

// ErrHelp is ParseArgs's error for --help, which prints Usage and succeeds.
var ErrHelp = flag.ErrHelp

// ParseArgs reads a command line without the program name. At most one of
// --settings, --background (or --hidden), and --mode may be given.
func ParseArgs(args []string) (Options, error) {
	set := flag.NewFlagSet("tinydash", flag.ContinueOnError)
	set.SetOutput(io.Discard)
	var opts Options
	var hidden, background bool
	var mode string
	set.StringVar(&opts.DataDir, "data-dir", "", "")
	set.BoolVar(&hidden, "hidden", false, "")
	set.BoolVar(&background, "background", false, "")
	set.BoolVar(&opts.Launch.Settings, "settings", false, "")
	set.StringVar(&mode, "mode", "", "")
	if err := set.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return opts, ErrHelp
		}
		return opts, fmt.Errorf("%v\n\n%s", err, Usage)
	}
	if set.NArg() > 0 {
		return opts, fmt.Errorf("Unknown argument “%s”.\n\n%s", set.Arg(0), Usage)
	}
	opts.Launch.Background = hidden || background
	modeSet := false
	set.Visit(func(f *flag.Flag) { modeSet = modeSet || f.Name == "mode" })
	if modeSet {
		if !slices.Contains(core.Categories(), core.Category(mode)) {
			return opts, fmt.Errorf("Unknown category “%s”.\n\n%s", mode, Usage)
		}
		opts.Launch.Category = core.Category(mode)
	}
	if countTrue(opts.Launch.Settings, opts.Launch.Background, modeSet) > 1 {
		return opts, errors.New(Usage)
	}
	return opts, nil
}

func countTrue(flags ...bool) int {
	n := 0
	for _, on := range flags {
		if on {
			n++
		}
	}
	return n
}

// Main runs TinyDash until it quits: one instance, no Dock icon, alive
// without windows. A second start hands its command line to the running
// TinyDash, which acts on it as the Rust app did.
func Main(opts Options) error {
	if !mygo.App.RequestSingleInstanceLock() {
		return nil
	}
	mygo.App.SetName("TinyDash")
	mygo.App.SetActivationPolicy(mygo.ActivationPolicyAccessory)
	mygo.App.OnWindowAllClosed(func() {})
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	var running *App
	mygo.App.OnSecondInstance(func(args []string, _ string) {
		if running == nil {
			return
		}
		again, err := ParseArgs(args)
		if err != nil {
			slog.Warn("second start", "error", err)
			return
		}
		open(running, again.Launch, true)
	})
	mygo.App.OnActivate(func(bool) {
		if running != nil {
			reopen(running)
		}
	})
	mygo.App.WhenReady(func() {
		opts.Launch.Background = opts.Launch.Background || mygo.App.WasOpenedAtLogin()
		started, err := Start(ctx, opts)
		if err != nil {
			log.Fatal(err)
		}
		running = started
	})
	return mygo.App.Run()
}

// reopen shows the launcher when the user opens the running app again,
// from Finder or Spotlight (macOS). Another app had the focus, which may
// have just hidden the launcher on blur, so a toggle could leave it
// hidden.
func reopen(app opener) { app.Show("") }

// opener is what a command line can ask of the running app.
type opener interface {
	toggle()
	Show(core.Category)
	openSettings() error
}

// open acts on a command line, as the Rust app's open did. Run again with
// no option, TinyDash toggles the launcher, so a desktop shortcut that runs
// tinydash works as the launcher shortcut does.
func open(app opener, launch Launch, alreadyRunning bool) {
	switch {
	case launch.Background:
	case launch.Settings:
		if err := app.openSettings(); err != nil {
			slog.Warn("open settings", "error", err)
		}
	case launch.Category == "" && alreadyRunning:
		app.toggle()
	default:
		app.Show(launch.Category)
	}
}
