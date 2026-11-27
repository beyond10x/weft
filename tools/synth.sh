#!/bin/sh
# Synthesize weft's Rust code from the specification into a cache under target/, or reuse it.
#
# Usage: tools/synth.sh [--spec <dir>] [--out <dir>]     default: --spec ess --out target/generated
#        tools/synth.sh --digest [--spec <dir>]          print the digest of the sources; calls no ess
#
# The generated code is not committed. <out> is a cache keyed by the digest of its sources,
# <out>/SOURCES.sha256: the line `ess <release>` (the `requires:` of <spec>/ess-inputs.yaml), then
# `<sha256>  <path>` for every file of <spec> in C order, then for tools/synth-input.sh and for
# tools/synth.sh itself, the two scripts the synthesis runs.
#   - <out> whose SOURCES.sha256 equals the digest and whose files match <out>/OUTPUT.sha256 (the
#     SHA-256 of every other file, and no file more) is reused without calling ess;
#   - any other <out> (stale: its recorded digest differs; damaged: a file edited, added or
#     missing) is never used: the code is synthesized again.
# Synthesis: tools/synth-input.sh derives the input (docs/implementation.md § Gaps E1-E3),
# `ess generate synthesize --target rust` writes it, the state file .ess-output/ is dropped (a
# random anchor id, read by nothing), and the two digest files are written. All of it happens in
# <out>.tmp.<pid>, renamed into place only after a successful synthesis, so a failed or killed run
# leaves no partial <out>. A lock file, <out>.lock holding the holder's pid, keeps two runs from
# writing the same cache: the second waits (at most WEFT_SYNTH_LOCK_WAIT seconds, default 300) and
# then finds the cache the first wrote. The lock is taken by the shell's noclobber redirection
# (take() below), never by mkdir: uutils mkdir, the mkdir of the Ubuntu 26.04 runner, lets two
# `mkdir d` started together both succeed. A lock is removed only by its holder (cleanup(), which
# checks the pid) or, when its holder is gone, by stale() below; the <out>.tmp.* and <out>.old.* a
# killed run left are removed by the next holder.
#
# ess compiles nothing, so it runs without slot (~/beyond10x/AGENTS.md "Scripts that do not build
# stay off slot"); it takes 9 to 30 s on ess/. It is ended after 600 s, and with this run however
# the run ends (a signal to its process group, its EXIT trap, or a watcher that sees it gone after
# a SIGKILL). Another ess release than the one <spec> requires is refused. For the self-test
# (tools/test-generated.sh): WEFT_SYNTH_ESS names the ess command, WEFT_SYNTH_INPUT another file
# used as tools/synth-input.sh. Paths are relative to the repository root, as given.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
self_file=$(cd "$(dirname "$0")" && pwd)/${0##*/}
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
  h=$(sha256sum < "$self_file" | cut -d' ' -f1)
  echo "$h  tools/synth.sh"
}

# The files of a tree other than its two digest files, one relative path per line, C order.
payload() {
  (cd "$1" && find . -type f ! -path ./SOURCES.sha256 ! -path ./OUTPUT.sha256 | sed 's|^\./||' | LC_ALL=C sort)
}

# Why the cache <out> may not be used, or nothing when it may.
unusable() {
  [ -d "$out" ] || { echo "no cache at $out"; return; }
  cmp -s "$out/SOURCES.sha256" "$tmp/want" || { echo "the cache at $out is stale: its SOURCES.sha256 differs from the sources"; return; }
  [ -f "$out/OUTPUT.sha256" ] || { echo "the cache at $out is damaged: no OUTPUT.sha256"; return; }
  payload "$out" > "$tmp/files"
  sed 's/^[0-9a-f]\{64\}  //' "$out/OUTPUT.sha256" | cmp -s - "$tmp/files" ||
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
ess_pid=

# take <file>: create <file> holding this run's pid, and fail when <file> exists. The shell opens
# it with O_EXCL under noclobber (dash and bash do), so of two runs only one succeeds; mkdir gives
# no such guarantee (uutils mkdir, see above).
take() { (set -C; echo $$ > "$1") 2>/dev/null; }

# pid_of <lock>: the pid in a lock, or nothing. A directory is a lock of an older tools/synth.sh,
# with its pid in <lock>/pid.
pid_of() {
  if [ -d "$1" ]; then cat "$1/pid"; else cat "$1"; fi 2>/dev/null || true
}

# The holder's exit: the lock is removed only while it still holds this run's pid.
cleanup() {
  [ -z "$ess_pid" ] || kill -TERM "$ess_pid" 2>/dev/null || true
  [ -z "$tmp" ] || rm -rf "$tmp"
  [ "$(pid_of "$lock")" != "$$" ] || rm -f "$lock"
}

# stale <pid>: remove the lock when it still is the one of <pid>, a holder that is gone (or, for
# an empty <pid>, a run killed between taking the lock and writing its pid, a minute ago or more).
# Only a run holding <lock>.steal removes a lock, and it reads the pid again under it: no other run
# can change a lock whose holder is gone, so the lock it removes is the one it read. A <lock>.steal
# a minute old or more was left by a run killed inside these lines and is removed.
stale() {
  if ! take "$lock.steal"; then
    if [ -n "$(find "$lock.steal" -maxdepth 0 -mmin +1 2>/dev/null)" ]; then rm -rf "$lock.steal"; fi
    sleep 1
    return 0
  fi
  again=$(pid_of "$lock")
  if [ -e "$lock" ] && [ "$again" = "$1" ]; then
    if [ -n "$1" ] && ! kill -0 "$1" 2>/dev/null; then
      mv "$lock" "$lock.stale.$$" && rm -rf "$lock.stale.$$" && echo "synth: removed the lock of $1, which is gone"
    elif [ -z "$1" ] && [ -n "$(find "$lock" -maxdepth 0 -mmin +1 2>/dev/null)" ]; then
      mv "$lock" "$lock.stale.$$" && rm -rf "$lock.stale.$$" && echo "synth: removed a lock without a pid, a minute old"
    fi
  fi
  rm -f "$lock.steal"
}

mkdir -p "$(dirname "$out")"
waited=0
while ! take "$lock"; do
  holder=$(pid_of "$lock")
  if { [ -n "$holder" ] && ! kill -0 "$holder" 2>/dev/null; } ||
    { [ -z "$holder" ] && [ -n "$(find "$lock" -maxdepth 0 -mmin +1 2>/dev/null)" ]; }; then
    stale "$holder"
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
rm -rf "$out".tmp.* "$out".old.*
tmp=$out.tmp.$$
mkdir -p "$tmp"

digest > "$tmp/want"
why=$(unusable)
if [ -z "$why" ]; then
  echo "synth: cache hit at $out"
  exit 0
fi
echo "synth: $why; synthesizing"

want=$(release)
have=$("$ess" --version | cut -d' ' -f2)
[ "$have" = "$want" ] || { echo "synth: ess is $have, $spec/ess-inputs.yaml requires $want" >&2; exit 1; }

"$synth_input" "$spec" "$tmp/input"
start=$(date +%s)
# ess runs in this run's process group, so a signal to the group (a `timeout` around this script)
# reaches it; a watcher ends it after 600 s, and as soon as this run is gone (SIGKILL included);
# the EXIT trap ends it on any other exit.
"$ess" generate synthesize --path "$tmp/input" --target rust --out "$tmp/output" </dev/null &
ess_pid=$!
self=$$
(
  n=0
  while kill -0 "$ess_pid" 2>/dev/null; do
    if ! kill -0 "$self" 2>/dev/null || [ "$n" -ge 600 ]; then
      kill -TERM "$ess_pid" 2>/dev/null
      sleep 5
      kill -KILL "$ess_pid" 2>/dev/null
      exit 0
    fi
    sleep 1
    n=$((n + 1))
  done
) </dev/null >/dev/null 2>&1 &
watcher=$!
ess_rc=0
wait "$ess_pid" || ess_rc=$?
ess_pid=
kill "$watcher" 2>/dev/null || true
[ "$ess_rc" = 0 ] || { echo "synth: ess generate synthesize failed with exit $ess_rc" >&2; exit "$ess_rc"; }
echo "synth: ess generate synthesize took $(($(date +%s) - start)) s"
rm -rf "$tmp/output/.ess-output"
cp "$tmp/want" "$tmp/output/SOURCES.sha256"
payload "$tmp/output" > "$tmp/files"
tr '\n' '\0' < "$tmp/files" | (cd "$tmp/output" && xargs -0 sha256sum) > "$tmp/OUTPUT.sha256"
mv "$tmp/OUTPUT.sha256" "$tmp/output/OUTPUT.sha256"

if [ -d "$out" ]; then mv "$out" "$out.old.$$"; fi
mv "$tmp/output" "$out"
rm -rf "$out.old.$$"
echo "synth: wrote $out ($(payload "$out" | wc -l) files)"
