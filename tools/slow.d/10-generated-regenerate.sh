#!/bin/sh
# Slow hook: synthesize the generated code afresh and compare bytes with the cache.
#
# Usage: tools/slow.d/10-generated-regenerate.sh [<spec dir> [<cache dir>]]
#        default: ess target/generated
#
# Makes sure the cache exists (tools/synth.sh, which reuses a cache whose digests hold), then
# synthesizes into a fresh directory beside it, <cache dir>.fresh.<pid>, and fails when any file
# differs or is missing on one side, naming each: a cache whose own digests hold can still be
# the output of another ess build, or of a synthesis that is not deterministic. Its own fresh
# directory is removed however the run ends but SIGKILL; at the start, those of runs that are gone
# (<cache dir>.fresh.<pid>, and the .tmp.*, .lock and .diff beside it) are removed. Paths are
# relative to the repository root. yq is installed pinned where missing (tools/synth-input.sh);
# ess must be the release the specification requires.
set -u

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 2
spec=${1:-ess}
cache=${2:-target/generated}
fresh=$cache.fresh.$$

for d in "$cache".fresh.*; do
  [ -e "$d" ] || continue
  pid=${d#"$cache".fresh.}
  pid=${pid%%.*}
  case $pid in '' | *[!0-9]*) continue ;; esac
  [ "$pid" != $$ ] && ! kill -0 "$pid" 2>/dev/null || continue
  rm -rf "$cache.fresh.$pid" "$cache.fresh.$pid".*
  echo "generated-regenerate: removed $cache.fresh.$pid of a run that is gone"
done

trap 'rm -rf "$fresh" "$fresh".*' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

tools/synth.sh --spec "$spec" --out "$cache" || { echo "generated-regenerate: the cache could not be made" >&2; exit 1; }
tools/synth.sh --spec "$spec" --out "$fresh" || { echo "generated-regenerate: the fresh synthesis failed" >&2; exit 1; }
if diff -r -q "$cache" "$fresh" > "$fresh.diff" 2>&1; then
  echo "generated-regenerate: $cache is byte-equal to a fresh synthesis from $spec"
  exit 0
fi
echo "generated-regenerate: $cache differs from a fresh synthesis from $spec:" >&2
sed "s|$fresh/||g; s|$cache/||g; s|^|  |" "$fresh.diff" >&2
exit 1
