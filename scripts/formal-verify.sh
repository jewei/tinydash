#!/usr/bin/env bash
# Model-check the TLA+ models in formal/ with TLC. Each model has a .cfg that
# must pass and a Broken .cfg that must fail on its invariant, which shows
# the check can catch the bug it guards. Needs Java 11 or later; downloads
# the pinned TLC once into a cache folder.
set -euo pipefail
cd "$(dirname "$0")/.."

version=1.7.4
sha256=936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88
cache="${XDG_CACHE_HOME:-$HOME/.cache}/tinydash"
jar="${TLA2TOOLS:-$cache/tla2tools-$version.jar}"
if [ ! -f "$jar" ]; then
  mkdir -p "$cache"
  curl -sSfL -o "$jar.download" "https://github.com/tlaplus/tlaplus/releases/download/v$version/tla2tools.jar"
  mv "$jar.download" "$jar"
fi
if ! echo "$sha256  $jar" | shasum -a 256 -c - >/dev/null 2>&1; then
  echo "TLC jar at $jar does not have the pinned SHA-256; delete it and run again." >&2
  exit 1
fi
java="${JAVA:-java}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

status=0
for spec in formal/*.tla; do
  name="$(basename "$spec" .tla)"
  for config in "formal/$name.cfg" "formal/${name}Broken.cfg"; do
    [ -f "$config" ] || continue
    out="$work/$(basename "$config").out"
    "$java" -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers auto -metadir "$work/meta-$(basename "$config")" \
      -config "$config" "$spec" >"$out" 2>&1 && passed=1 || passed=0
    if [[ "$config" == *Broken.cfg ]]; then
      if [ $passed = 0 ] && grep -q "is violated" "$out"; then
        echo "ok    $config fails as it must: $(grep -m1 'is violated' "$out" | sed 's/^Error: //')"
      else
        echo "FAIL  $config should break its invariant but did not" >&2; tail -20 "$out" >&2; status=1
      fi
    elif [ $passed = 1 ]; then
      echo "ok    $config: $(grep -m1 'distinct states found' "$out")"
    else
      echo "FAIL  $config" >&2; grep -E 'violated|Error' "$out" >&2 || tail -20 "$out" >&2; status=1
    fi
  done
done
exit $status
