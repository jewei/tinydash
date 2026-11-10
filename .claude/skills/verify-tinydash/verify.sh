#!/usr/bin/env bash
# Drive TinyDash (the Go port) inside a Linux container on a virtual X
# display. Nothing here touches the Mac's desktop, clipboard, or TinyDash
# data. Run from anywhere in the repo; see SKILL.md for the journeys.
#
#   verify.sh build                 image + linux binaries (e2e build) for this run
#   verify.sh launch                start a box, seed it, start the app, wait
#   verify.sh doctor                read-only health check of the running box
#   verify.sh auto <args...>        mygo-automation inside the box
#   verify.sh ref <text>            the ref of the first snapshot line holding text
#   verify.sh show                  the real shortcut shows the panel; waits for focused
#   verify.sh hide                  the real shortcut hides the panel; waits for hidden
#   verify.sh box <shell command>   a shell command inside the box (xclip, sqlite3, ...)
#   verify.sh sql <query>           sqlite3 on the app's tinydash.db
#   verify.sh evidence <name>       screenshot + snapshot + windows into the run folder
#   verify.sh cleanup               save the app log, stop the box; evidence stays
#
# TD_RUN names the run (default: local). Runs side by side need their own.
set -euo pipefail

repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
skill="$repo/.claude/skills/verify-tinydash"
root="$repo/.local/verify"
run="${TD_RUN:-local}"
[[ "$run" =~ ^[A-Za-z0-9_.-]+$ ]] || { echo "TD_RUN must be letters, digits, . _ -" >&2; exit 2; }
box="tinydash-verify-$run"
image="tinydash-verify"
evidence="$root/runs/$run"
# Each run has its own binaries, so a build for one run never changes the
# app that another run's box is running.
bin="$root/builds/$run"

in_box() { docker exec -e MYGO_AUTOMATION_SESSION="$run" "$box" "$@"; }

# press_shortcut presses the real launcher shortcut on the virtual display
# and polls the panel until its window line holds want (focused, hidden).
press_shortcut() {
  local want="$1" _
  in_box xdotool key ctrl+shift+space
  for _ in $(seq 50); do
    if in_box /opt/td/mygo-automation windows | grep -q "\"TinyDash\" $want"; then
      in_box /opt/td/mygo-automation windows
      return 0
    fi
    sleep 0.1
  done
  echo "the panel was not $want 5 s after the shortcut:" >&2
  in_box /opt/td/mygo-automation windows >&2
  return 1
}

running() { [[ "$(docker inspect -f '{{.State.Running}}' "$box" 2>/dev/null)" == true ]]; }

case "${1:-}" in
build)
  docker build -q -t "$image" "$skill" >/dev/null
  arch="$(docker version --format '{{.Server.Arch}}')"
  case "$arch" in aarch64) arch=arm64 ;; x86_64) arch=amd64 ;; esac
  mkdir -p "$bin"
  (cd "$repo" && GOOS=linux GOARCH="$arch" CGO_ENABLED=0 go build -tags e2e -o "$bin/tinydash" ./cmd/tinydash)
  # From the app's module, so the client is the mygo version go.mod pins.
  (cd "$repo" && GOOS=linux GOARCH="$arch" CGO_ENABLED=0 go build -o "$bin/mygo-automation" github.com/egoist/mygo/plugins/automation/cmd/mygo-automation)
  {
    echo "commit $(git -C "$repo" describe --always --dirty --abbrev=12)"
    echo "sha256 $(shasum -a 256 "$bin/tinydash" | cut -d' ' -f1)"
    echo "arch linux/$arch"
  } >"$bin/BUILD"
  cat "$bin/BUILD"
  ;;
launch)
  [[ -x "$bin/tinydash" ]] || { echo "no build for $run: run verify.sh build with this TD_RUN" >&2; exit 1; }
  if docker inspect "$box" >/dev/null 2>&1; then
    echo "$box exists: run verify.sh cleanup first, or set another TD_RUN" >&2
    exit 1
  fi
  mkdir -p "$evidence" "$root/cache"
  cp "$bin/BUILD" "$evidence/BUILD"
  # cache: mygo's SQLite library, downloaded once and SHA-256-checked.
  # --init reaps exited processes, so a pid that ended is gone, not a zombie.
  docker run -d --rm --init --name "$box" --label tinydash-verify="$run" \
    -v "$bin:/opt/td:ro" -v "$skill:/opt/skill:ro" \
    -v "$root/cache:/cache" -v "$evidence:/evidence" \
    -e XDG_CACHE_HOME=/cache -e DISPLAY=:99 \
    "$image" sleep infinity >/dev/null
  in_box /opt/skill/box.sh start
  echo "ready: $box, evidence in ${evidence#"$repo/"}"
  ;;
doctor)
  running || { echo "FAIL container $box is not running" >&2; exit 1; }
  echo "ok   container $box"
  in_box /opt/skill/box.sh doctor
  want="$(git -C "$repo" describe --always --dirty --abbrev=12)"
  built="$(sed -n 's/^commit //p' "$bin/BUILD")"
  if [[ "$built" == "$want" ]]; then
    echo "ok   build $built is this checkout"
  else
    echo "WARN build $built, checkout $want: run verify.sh build, then launch again"
  fi
  ;;
auto)
  shift
  in_box /opt/td/mygo-automation "$@"
  ;;
ref)
  shift
  line="$(in_box /opt/td/mygo-automation snapshot | grep -F -m1 -- "$*" || true)"
  [[ -n "$line" ]] || { echo "no element line holds: $*" >&2; exit 1; }
  sed -E 's/^ *(@e[0-9]+).*/\1/' <<<"$line"
  ;;
show)
  if in_box /opt/td/mygo-automation windows | grep -q '"TinyDash" focused'; then
    in_box /opt/td/mygo-automation windows
    exit 0
  fi
  # The app takes a shortcut press within blurWithin (400 ms) of a hide by
  # blur as the cause of that blur, and keeps the panel hidden. Any hide,
  # by Enter, a digit, or the shortcut, may have just happened, so wait it
  # out here: then no journey needs to know about the window.
  sleep 0.5
  press_shortcut focused
  ;;
hide)
  press_shortcut hidden
  ;;
box)
  shift
  in_box sh -c "$*"
  ;;
sql)
  shift
  in_box sqlite3 -readonly -header /scratch/data/tinydash.db "$*"
  ;;
evidence)
  name="${2:?usage: verify.sh evidence <name>}"
  [[ "$name" =~ ^[A-Za-z0-9_.-]+$ ]] || { echo "name must be letters, digits, . _ -" >&2; exit 2; }
  in_box /opt/td/mygo-automation screenshot "/evidence/$name.png"
  in_box /opt/td/mygo-automation snapshot >"$evidence/$name.tree.txt"
  in_box /opt/td/mygo-automation windows >"$evidence/$name.windows.txt"
  echo "${evidence#"$repo/"}/$name.{png,tree.txt,windows.txt}"
  ;;
cleanup)
  if running; then
    in_box sh -c 'cp /scratch/app.log /evidence/app.log' || true
    docker stop -t 5 "$box" >/dev/null
    echo "stopped $box"
  else
    echo "no running $box"
  fi
  echo "evidence kept in ${evidence#"$repo/"}"
  ;;
*)
  sed -n '5,18p' "$0" >&2
  exit 2
  ;;
esac
