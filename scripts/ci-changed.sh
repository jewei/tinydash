#!/usr/bin/env bash
# Which Go jobs a changed-file list needs.
#
# stdin is one path per line, as `git diff --name-only --no-renames` prints.
# $1 is the event name: pull_request or push.
# stdout is three lines for $GITHUB_OUTPUT: code, darwin, and windows,
# each true or false.
#
# Linux runs whenever code is true. macOS runs when darwin is true, Windows
# when windows is true. A push with code runs every OS, so a pull request
# that skipped an OS is still tested once the commit is on main.
# An empty list counts as code on every OS: a diff we could not read.
#
# `bash scripts/ci-changed.sh --check` checks these rules against this tree.
# The CI changes job runs that before it trusts the flags.
#
# A file's //go:build line is read from CI_CHANGED_ROOT (a local checkout),
# otherwise from git show of CI_CHANGED_SHA and then CI_CHANGED_BASE, so a
# deleted file is still classified. An expression this script does not
# recognize that names an OS runs both extra OS jobs, so a new tag is tested
# rather than skipped.

set -eu

bool() {
  if [ "$1" = 1 ]; then
    echo true
  else
    echo false
  fi
}

ignored() {
  case "$1" in
    docs/architecture.md) return 1 ;;
    *.md | docs/* | LICENSE | .gitignore) return 0 ;;
    .claude/* | .vite-hooks/* | .github/dependabot.yml) return 0 ;;
    .github/workflows/*)
      [ "$1" = ".github/workflows/ci.yml" ] && return 1
      return 0
      ;;
    *.png | *.avif | *.svg | *.ico | *.icns | *.gif | *.webp | *.jpg | *.jpeg) return 0 ;;
    *) return 1 ;;
  esac
}

# Dependencies and the files that decide how CI itself runs. A pull request
# that changes these runs every OS, because the failure would not show on
# Linux alone.
all_os_path() {
  case "$1" in
    go.mod | go.sum | go.work | go.work.sum | mygo.json | .github/workflows/ci.yml | scripts/verify-go.sh | scripts/ci-changed.sh)
      return 0
      ;;
    *) return 1 ;;
  esac
}

file_head() {
  path="$1"
  if [ -n "${CI_CHANGED_ROOT:-}" ] && [ -f "$CI_CHANGED_ROOT/$path" ]; then
    head -n 40 "$CI_CHANGED_ROOT/$path" 2>/dev/null || true
    return
  fi
  if [ -n "${CI_CHANGED_SHA:-}" ] && git cat-file -e "$CI_CHANGED_SHA:$path" 2>/dev/null; then
    git show "$CI_CHANGED_SHA:$path" 2>/dev/null | head -n 40 || true
    return
  fi
  if [ -n "${CI_CHANGED_BASE:-}" ] && git cat-file -e "$CI_CHANGED_BASE:$path" 2>/dev/null; then
    git show "$CI_CHANGED_BASE:$path" 2>/dev/null | head -n 40 || true
    return
  fi
  if [ -f "$path" ]; then
    head -n 40 "$path" 2>/dev/null || true
  fi
}

# Sets darwin_os and windows_os for a Go file from its name and build tag.
# The _linux/_darwin/_windows file name is the constraint Go itself uses,
# including when the file has no //go:build line.
apply_go() {
  path="$1"
  base=$(basename "$path")
  case "$base" in
    *_darwin.go | *_darwin_test.go) darwin_os=1; return ;;
    *_windows.go | *_windows_test.go) windows_os=1; return ;;
    *_linux.go | *_linux_test.go) return ;;
    labels.go)
      # runtime.GOOS is read only in labels.go, outside the platform files.
      darwin_os=1
      windows_os=1
      return
      ;;
  esac

  raw=$(file_head "$path" | tr -d '\r' | sed -n 's|^//go:build ||p; s|^// +build ||p' | head -n 1 || true)
  expr=${raw// /}
  under=0
  case "$path" in
    internal/platform/*) under=1 ;;
  esac
  if [ -z "$expr" ]; then
    # A platform file with no OS in its name is compiled into every OS.
    if [ "$under" = 1 ]; then
      darwin_os=1
      windows_os=1
    fi
    return
  fi
  case "$expr" in
    darwin) darwin_os=1 ;;
    windows) windows_os=1 ;;
    linux) ;;
    !darwin) windows_os=1 ;;
    !windows) darwin_os=1 ;;
    !linux) darwin_os=1; windows_os=1 ;;
    *)
      if [[ "$expr" == *darwin* || "$expr" == *windows* || "$expr" == *linux* ]]; then
        darwin_os=1
        windows_os=1
      elif [ "$under" = 1 ] && [ "$expr" != e2e ]; then
        darwin_os=1
        windows_os=1
      fi
      ;;
  esac
}

# classify EVENT PATH...
# Sets code, darwin_os, and windows_os.
classify() {
  event="$1"
  shift
  code=0
  darwin_os=0
  windows_os=0
  saw=0
  for path in "$@"; do
    saw=1
    if ignored "$path"; then
      continue
    fi
    code=1
    if all_os_path "$path"; then
      darwin_os=1
      windows_os=1
      continue
    fi
    case "$path" in
      Entitlements.plist) darwin_os=1; continue ;;
    esac
    case "$path" in
      *.go) apply_go "$path" ;;
    esac
  done
  if [ "$saw" = 0 ]; then
    code=1
    darwin_os=1
    windows_os=1
  fi
  # push is only main. Run the OS jobs the pull request was allowed to skip.
  if [ "$event" = push ] && [ "$code" = 1 ]; then
    darwin_os=1
    windows_os=1
  fi
}

emit() {
  echo "code=$(bool "$code")"
  echo "darwin=$(bool "$darwin_os")"
  echo "windows=$(bool "$windows_os")"
}

check_one() {
  want_event="$1"
  want_code="$2"
  want_darwin="$3"
  want_windows="$4"
  shift 4
  classify "$want_event" "$@"
  if [ "$(bool "$code")" != "$want_code" ] || [ "$(bool "$darwin_os")" != "$want_darwin" ] || [ "$(bool "$windows_os")" != "$want_windows" ]; then
    echo "ci-changed: $* ($want_event) is code=$(bool "$code") darwin=$(bool "$darwin_os") windows=$(bool "$windows_os"), want code=$want_code darwin=$want_darwin windows=$want_windows" >&2
    check_fail=1
  fi
}

self_check() {
  root="${CI_CHANGED_ROOT:-.}"
  check_fail=0
  # file_head reads CI_CHANGED_ROOT. Stay on this tree, not git show.
  CI_CHANGED_ROOT="$root"
  unset CI_CHANGED_SHA CI_CHANGED_BASE || true
  while IFS= read -r rel; do
    base=$(basename "$rel")
    case "$base" in
      *_darwin.go | *_darwin_test.go)
        check_one pull_request true true false "$rel" ;;
      *_windows.go | *_windows_test.go)
        check_one pull_request true false true "$rel" ;;
      *_linux.go | *_linux_test.go)
        check_one pull_request true false false "$rel" ;;
      labels.go)
        check_one pull_request true true true "$rel" ;;
    esac
  done < <(cd "$root" && find . -name '*.go' -print | sed 's|^\./||')
  check_one pull_request true false true internal/platform/find_other.go
  check_one pull_request true false true internal/platform/secret_other.go
  check_one pull_request true true true internal/platform/windowclass_other.go
  check_one pull_request true true false internal/platform/zone_other.go
  check_one pull_request true true true internal/platform/calendar.go
  check_one pull_request true false false internal/search/personal.go
  check_one pull_request true false false internal/app/automation_e2e.go
  check_one pull_request false false false README.md
  check_one pull_request false false false docs/features.md LICENSE
  check_one pull_request true false false docs/architecture.md
  check_one pull_request true true true go.mod
  check_one pull_request true true false Entitlements.plist
  check_one pull_request false false false .github/workflows/codeql.yml
  check_one pull_request true true false internal/platform/clipboard_darwin.go README.md
  check_one pull_request true true true internal/platform/windowclass_linux.go internal/platform/windowclass_other.go
  check_one push true true true internal/search/personal.go
  check_one push false false false README.md
  if [ "$check_fail" != 0 ]; then
    exit 1
  fi
}

if [ "${1:-}" = --check ]; then
  self_check
  exit 0
fi

event="${1:-pull_request}"
files=()
while IFS= read -r path || [ -n "$path" ]; do
  path=${path%$'\r'}
  [ -z "$path" ] && continue
  files+=("$path")
done
if [ "${#files[@]}" -eq 0 ]; then
  classify "$event"
else
  classify "$event" "${files[@]}"
fi
emit
