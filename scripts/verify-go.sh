#!/usr/bin/env bash
# Every Go gate in one command: vet on all three OSes, staticcheck, and the
# tests with the race detector (they include the import rules and gofmt).
# The tests run on this OS only; CI runs them on macOS, Windows, and Linux.
set -euo pipefail
for os in darwin windows linux; do
  GOOS=$os go vet ./...
done
# Build staticcheck once for this machine, then lint as each OS sees the
# code: a symbol used on only some OSes is unused on the others.
tools="$(mktemp -d)"
trap 'rm -rf "$tools"' EXIT
go build -o "$tools/staticcheck" honnef.co/go/tools/cmd/staticcheck
for os in darwin windows linux; do
  GOOS=$os "$tools/staticcheck" ./...
done
# -count=1: archtest reads the module through `go list`, which the test
# cache cannot see, so a cached pass could hide a new violation.
go test -race -count=1 ./...
