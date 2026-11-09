#!/usr/bin/env bash
# Model-check the TLA+ models in formal/ with TLC. Configs that model the
# code must pass; configs that model a known bug must fail, which shows the
# check can catch it. Needs Java 11 or later; downloads
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
# Each config is <Spec>.<variant>.cfg beside <Spec>.tla. A config whose name
# holds ".broken" models a known bug: it must fail on the invariant that its
# "\* must break: <Invariant>" line names, so a broken config that fails for
# another reason does not count. Every other config must pass.
for config in formal/*.cfg; do
  base="$(basename "$config" .cfg)"
  spec="formal/${base%%.*}.tla"
  out="$work/$base.out"
  "$java" -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers auto -metadir "$work/meta-$base" \
    -config "$config" "$spec" >"$out" 2>&1 && passed=1 || passed=0
  if [[ "$base" == *.broken* ]]; then
    want="$(sed -n 's/^\\\* must break: *\([A-Za-z0-9_]*\) *$/\1/p' "$config" | head -1)"
    if [ -z "$want" ]; then
      echo "FAIL  $config has no '\\* must break: <Invariant>' line" >&2; status=1
    elif [ $passed = 0 ] && grep -q "^Error: Invariant $want is violated" "$out"; then
      echo "ok    $config fails as it must: Invariant $want is violated."
    else
      echo "FAIL  $config should break $want but did not" >&2; grep -E 'violated|Error' "$out" >&2 || tail -20 "$out" >&2; status=1
    fi
  elif [ $passed = 1 ]; then
    echo "ok    $config: $(grep -m1 'distinct states found' "$out")"
  else
    echo "FAIL  $config" >&2; grep -E 'violated|Error' "$out" >&2 || tail -20 "$out" >&2; status=1
  fi
done
exit $status
