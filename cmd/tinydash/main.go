// TinyDash is a keyboard launcher. Run it with --data-dir to keep its data
// in a folder of your choice, which development and verification do so
// that they never touch the user's data. It takes the Rust app's options
// too: --settings, --background, and --mode <category>.
package main

import (
	"errors"
	"fmt"
	"log"
	"os"

	"github.com/jewei/tinydash/internal/app"
)

func main() {
	opts, err := app.ParseArgs(os.Args[1:])
	if errors.Is(err, app.ErrHelp) {
		fmt.Println(app.Usage)
		return
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(2)
	}
	if err := app.Main(opts); err != nil {
		log.Fatal(err)
	}
}
