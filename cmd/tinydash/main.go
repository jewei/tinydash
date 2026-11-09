// TinyDash is a keyboard launcher. Run it with --data-dir to keep its data
// in a folder of your choice, which development and verification do so
// that they never touch the user's data.
package main

import (
	"flag"
	"log"

	"github.com/jewei/tinydash/internal/app"
)

func main() {
	dataDir := flag.String("data-dir", "", "keep settings.json and tinydash.db in this folder")
	hidden := flag.Bool("hidden", false, "start without showing the launcher")
	flag.Parse()
	if err := app.Main(app.Options{DataDir: *dataDir, Hidden: *hidden}); err != nil {
		log.Fatal(err)
	}
}
