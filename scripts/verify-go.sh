#!/usr/bin/env bash
# Every Go gate in one command: vet on all three OSes, staticcheck, and the
# tests with the race detector (they include the import rules and gofmt).
set -euo pipefail
for os in darwin windows linux; do
  GOOS=$os go vet ./...
done
go tool staticcheck ./...
# -count=1: archtest reads the module through `go list`, which the test
# cache cannot see, so a cached pass could hide a new violation.
go test -race -count=1 ./...
