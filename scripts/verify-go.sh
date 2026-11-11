#!/usr/bin/env bash
# Every Go gate in one command: vet on all three OSes, staticcheck, the
# tests with the race detector (they include the import rules and gofmt),
# and the search speed gate.
# The tests run on this OS only. CI runs them on Linux for every code
# change, and on macOS and Windows when that OS is affected or on main.
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
# COVERAGE_OUT is set by CI on Linux. -race needs the atomic cover mode.
# The speed gate below stays a separate run, without the cover profile.
# Two commands, not an empty array: macOS CI runs this file under bash 3.2.
if [ -n "${COVERAGE_OUT:-}" ]; then
  go test -race -count=1 -coverprofile="$COVERAGE_OUT" -covermode=atomic ./...
else
  go test -race -count=1 ./...
fi
# Search speed: TestSearchStaysWithinBudget skips under the race detector,
# which slows search several times, so it runs again here without it.
go test -count=1 -run '^TestSearchStaysWithinBudget$' ./internal/search
