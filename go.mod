module github.com/jewei/tinydash

go 1.27.3

// The Rust and TypeScript app lives beside the Go code until the port ends.
ignore (
	./dist
	./node_modules
	./src
	./src-tauri
)

require github.com/ebitengine/purego v0.11.1

require (
	github.com/BurntSushi/toml v1.4.1-0.20240526193622-a339e1f7089c // indirect
	github.com/egoist/mygo v0.8.3 // indirect
	golang.org/x/exp/typeparams v0.0.0-20231108232855-2478ac86f678 // indirect
	golang.org/x/mod v0.41.0 // indirect
	golang.org/x/sync v0.24.0 // indirect
	golang.org/x/text v0.43.0 // indirect
	golang.org/x/tools v0.51.0 // indirect
	honnef.co/go/tools v0.8.2 // indirect
)

tool (
	github.com/egoist/mygo/cmd/mygo
	honnef.co/go/tools/cmd/staticcheck
)
