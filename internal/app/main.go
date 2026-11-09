package app

import (
	"context"
	"log"

	"github.com/egoist/mygo"
)

// Main runs TinyDash until it quits: one instance, no Dock icon, alive
// without windows. A second start shows the running launcher.
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
	mygo.App.OnSecondInstance(func([]string, string) {
		if running != nil {
			running.Show()
		}
	})
	mygo.App.WhenReady(func() {
		opts.Hidden = opts.Hidden || mygo.App.WasOpenedAtLogin()
		started, err := Start(ctx, opts)
		if err != nil {
			log.Fatal(err)
		}
		running = started
	})
	return mygo.App.Run()
}
