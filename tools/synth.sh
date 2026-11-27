#!/bin/sh
# Synthesize weft's Rust code from the specification into a cache under target/, or reuse it.
#
# Usage: tools/synth.sh [--spec <dir>] [--out <dir>]     default: --spec ess --out target/generated
#        tools/synth.sh --digest [--spec <dir>]          print the digest of the sources; calls no ess
#
# The generated code is not committed. <out> is a cache keyed by the digest of its sources,
# <out>/SOURCES.sha256: the line `ess <release>` (the `requires:` of <spec>/ess-inputs.yaml), then
# `<sha256>  <path>` for every file of <spec> in C order, then for tools/synth-input.sh.
#   - <out> whose SOURCES.sha256 equals the digest and whose files match <out>/OUTPUT.sha256 (the
#     SHA-256 of every other file, and no file more) is reused without calling ess;
#   - any other <out> (stale: its recorded digest differs; damaged: a file edited, added or
#     missing) is never used: the code is synthesized again.
# Synthesis: tools/synth-input.sh derives the input (docs/implementation.md § Gaps E1-E3),
# `ess generate synthesize --target rust` writes it, the state file .ess-output/ is dropped (a
# random anchor id, read by nothing), and the two digest files are written. All of it happens in
# <out>.tmp.<pid>, renamed into place only after a successful synthesis, so a failed or killed run
# leaves no partial <out>. A lock directory, <out>.lock with the holder's pid, keeps two runs from
# writing the same cache: the second waits (at most WEFT_SYNTH_LOCK_WAIT seconds, default 300) and
# then finds the cache the first wrote. A lock whose holder is gone is removed, and so are the
# <out>.tmp.* and <out>.old.* a killed run left.
#
# ess compiles nothing, so it runs without slot (~/beyond10x/AGENTS.md "Scripts that do not build
# stay off slot"), bounded at 600 s; it takes 9 to 30 s on ess/. Another ess release than the one
# <spec> requires is refused. For the self-test (tools/test-generated.sh): WEFT_SYNTH_ESS names
# the ess command, WEFT_SYNTH_INPUT another file used as tools/synth-input.sh. Paths are relative
# to the repository root, as given.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
synth_input=${WEFT_SYNTH_INPUT:-tools/synth-input.sh}
ess=${WEFT_SYNTH_ESS:-ess}
lock_wait=${WEFT_SYNTH_LOCK_WAIT:-300}

mode=generate
spec=ess
out=target/generated
while [ "$#" -gt 0 ]; do
  case $1 in
    --digest) mode=digest; shift ;;
    --spec) spec=${2:?--spec needs a directory}; shift 2 ;;
    --out) out=${2:?--out needs a directory}; shift 2 ;;
    *) echo "usage: tools/synth.sh [--digest] [--spec <dir>] [--out <dir>]" >&2; exit 2 ;;
  esac
done
spec=${spec%/}
out=${out%/}
[ -d "$spec" ] || { echo "synth: no directory $spec" >&2; exit 2; }

release() {
  sed -n 's/^requires:[[:space:]]*ess[[:space:]]\{1,\}\([^[:space:]]*\).*/\1/p' "$spec/ess-inputs.yaml" | head -n 1
}

digest() {
  r=$(release)
  [ -n "$r" ] || { echo "synth: $spec/ess-inputs.yaml requires no ess release" >&2; return 1; }
  echo "ess $r"
  (cd "$spec" && find . -type f | sed 's|^\./||' | LC_ALL=C sort) | while IFS= read -r f; do
    h=$(sha256sum < "$spec/$f" | cut -d' ' -f1)
    echo "$h  $spec/$f"
  done
  h=$(sha256sum < "$synth_input" | cut -d' ' -f1)
  echo "$h  tools/synth-input.sh"
}

# The files of a tree other than its two digest files, one relative path per line, C order.
payload() {
  (cd "$1" && find . -type f ! -path ./SOURCES.sha256 ! -path ./OUTPUT.sha256 | sed 's|^\./||' | LC_ALL=C sort)
}

# Why the cache <out> may not be used, or nothing when it may.
unusable() {
  [ -d "$out" ] || { echo "no cache at $out"; return; }
  cmp -s "$out/SOURCES.sha256" "$lock/want" || { echo "the cache at $out is stale: its SOURCES.sha256 differs from the sources"; return; }
  [ -f "$out/OUTPUT.sha256" ] || { echo "the cache at $out is damaged: no OUTPUT.sha256"; return; }
  payload "$out" > "$lock/files"
  sed 's/^[0-9a-f]\{64\}  //' "$out/OUTPUT.sha256" | cmp -s - "$lock/files" ||
    { echo "the cache at $out is damaged: its files differ from OUTPUT.sha256"; return; }
  (cd "$out" && sha256sum --quiet --strict -c OUTPUT.sha256 >/dev/null 2>&1) ||
    { echo "the cache at $out is damaged: a file differs from OUTPUT.sha256"; return; }
}

if [ "$mode" = digest ]; then
  digest
  exit 0
fi

lock=$out.lock
tmp=
cleanup() {
  [ -z "$tmp" ] || rm -rf "$tmp"
  rm -rf "$lock"
}
mkdir -p "$(dirname "$out")"
waited=0
while ! mkdir "$lock" 2>/dev/null; do
  holder=$(cat "$lock/pid" 2>/dev/null || true)
  if [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; then
    if mv "$lock" "$lock.stale.$$" 2>/dev/null; then
      rm -rf "$lock.stale.$$"
      echo "synth: removed the lock of $holder, which is gone"
    fi
    continue
  fi
  if [ "$waited" -ge "$lock_wait" ]; then
    echo "synth: $lock is held by ${holder:-a starting run} for ${lock_wait} s" >&2
    exit 1
  fi
  [ "$waited" -gt 0 ] || echo "synth: waiting for $lock (held by ${holder:-a starting run})"
  sleep 1
  waited=$((waited + 1))
done
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
echo $$ > "$lock/pid"
rm -rf "$out".tmp.* "$out".old.*

digest > "$lock/want"
why=$(unusable)
if [ -z "$why" ]; then
  echo "synth: cache hit at $out"
  exit 0
fi
echo "synth: $why; synthesizing"

want=$(release)
have=$("$ess" --version | cut -d' ' -f2)
[ "$have" = "$want" ] || { echo "synth: ess is $have, $spec/ess-inputs.yaml requires $want" >&2; exit 1; }

tmp=$out.tmp.$$
mkdir -p "$tmp"
"$synth_input" "$spec" "$tmp/input"
start=$(date +%s)
timeout -k 5 600 "$ess" generate synthesize --path "$tmp/input" --target rust --out "$tmp/output" </dev/null
echo "synth: ess generate synthesize took $(($(date +%s) - start)) s"
rm -rf "$tmp/output/.ess-output"
cp "$lock/want" "$tmp/output/SOURCES.sha256"
payload "$tmp/output" > "$lock/files"
tr '\n' '\0' < "$lock/files" | (cd "$tmp/output" && xargs -0 sha256sum) > "$tmp/OUTPUT.sha256"
mv "$tmp/OUTPUT.sha256" "$tmp/output/OUTPUT.sha256"

if [ -d "$out" ]; then mv "$out" "$out.old.$$"; fi
mv "$tmp/output" "$out"
rm -rf "$out.old.$$"
echo "synth: wrote $out ($(payload "$out" | wc -l) files)"
