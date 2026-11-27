#!/bin/sh
# The test step of the gate (task test, run by `task check` locally and in CI).
# Usage: tools/test-step.sh [command...]     command default: cargo nextest run --workspace
#   1. refuses a WEFT_STEP_LIMIT outside 1..120;
#   2. prints `${RUSTC:-rustc} -V` and `cargo-nextest --version` and fails when either differs from
#      its pin: the channel of the root rust-toolchain.toml, nextest-version of .config/nextest.toml;
#   3. runs the setup hooks (tools/test-setup.d), then the tests; both count against
#      WEFT_STEP_LIMIT seconds (default below), and the step ends at the limit with exit 124;
#   4. runs the teardown hooks (tools/test-teardown.d, at most 30 s), also after a failed or
#      killed run and after SIGINT, SIGTERM or SIGHUP, which end the setup hooks or the tests first;
#   5. prints the ten slowest tests, time and name, from nextest's JUnit output and writes the same
#      lines to target/nextest/slowest.txt; a step without JUnit output fails.
# A step that ends past its limit, teardown included, fails. The test binaries are built before
# this step (cargo nextest run --workspace --no-run). tools/test-gate-budget.sh reads the default
# limit from this file and fails when it is over 120; the other variables exist for that self-test.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
max=120
teardown_limit=30
limit=${WEFT_STEP_LIMIT:-120}
toolchain_file=${WEFT_TOOLCHAIN_FILE:-$root/rust-toolchain.toml}
setup_dir=${WEFT_SETUP_DIR:-$root/tools/test-setup.d}
teardown_dir=${WEFT_TEARDOWN_DIR:-$root/tools/test-teardown.d}
junit=${WEFT_JUNIT:-$root/target/nextest/${NEXTEST_PROFILE:-default}/junit.xml}
slowest=${WEFT_SLOWEST:-$root/target/nextest/slowest.txt}
[ "$#" -gt 0 ] || set -- cargo nextest run --workspace

case $limit in
  '' | *[!0-9]*) limit_ok=no ;;
  *) if [ "$limit" -ge 1 ] 2>/dev/null && [ "$limit" -le "$max" ] 2>/dev/null; then limit_ok=yes; else limit_ok=no; fi ;;
esac
[ "$limit_ok" = yes ] || { echo "test-step: WEFT_STEP_LIMIT=$limit is outside 1..$max" >&2; exit 2; }

pin() { sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$2" | head -n 1; }
second() { echo "$1" | awk '{print $2}'; }
channel=$(pin channel "$toolchain_file")
version=$("${RUSTC:-rustc}" -V) || { echo "test-step: ${RUSTC:-rustc} -V failed" >&2; exit 1; }
echo "$version"
[ -n "$channel" ] && [ "$(second "$version")" = "$channel" ] || {
  echo "test-step: rustc is not the pinned channel \"$channel\" of $toolchain_file" >&2; exit 1; }
nextest_pin=$(pin nextest-version "$root/.config/nextest.toml")
nextest=$(cargo-nextest nextest --version 2>/dev/null | head -n 1)
echo "$nextest"
[ -n "$nextest_pin" ] && [ "$(second "$nextest")" = "$nextest_pin" ] || {
  echo "test-step: cargo-nextest is not nextest-version \"$nextest_pin\" of .config/nextest.toml" >&2; exit 1; }

# A signal ends the running phase (the timeout below runs in a process group of its own, so the
# signal is forwarded to it) and the step goes on to the teardown hooks.
child=
pending=
signalled=
forward() {
  signalled=$1
  if [ -n "$child" ]; then kill -TERM "$child" 2>/dev/null; else pending=1; fi
}
trap 'forward 1' HUP
trap 'forward 2' INT
trap 'forward 15' TERM

# run_bounded <seconds> <command...>: the command under timeout, in the background, waited on.
run_bounded() {
  secs=$1
  shift
  timeout -k 5 "$secs" "$@" &
  child=$!
  if [ -n "$pending" ]; then pending=; kill -TERM "$child" 2>/dev/null; fi
  wait "$child"
  r=$?
  while [ -n "$signalled" ] && kill -0 "$child" 2>/dev/null; do
    wait "$child"
    r=$?
  done
  child=
  return "$r"
}

start=$(date +%s)
left() { echo $((limit - ($(date +%s) - start))); }
past_limit() {
  [ "$(left)" -le 0 ] || return 1
  echo "test-step: the step ran past its limit of ${limit}s and was ended" >&2
}

rm -f "$junit"
rc=0
for phase in setup tests; do
  [ "$rc" = 0 ] && [ -z "$signalled" ] || break
  s=$(left)
  if [ "$s" -lt 1 ]; then rc=124; past_limit; break; fi
  if [ "$phase" = setup ]; then
    run_bounded "$s" "$root/tools/run-hooks.sh" "$setup_dir" || rc=$?
  else
    run_bounded "$s" "$@" || rc=$?
  fi
  case $rc in 124 | 137) if past_limit; then rc=124; fi ;; esac
done

pending=
run_bounded "$teardown_limit" "$root/tools/run-hooks.sh" "$teardown_dir" || { t=$?; [ "$rc" != 0 ] || rc=$t; }
if [ "$rc" = 0 ] && past_limit; then rc=124; fi
if [ -n "$signalled" ]; then
  echo "test-step: ended by signal $signalled" >&2
  rc=$((128 + signalled))
fi

if [ -f "$junit" ]; then
  mkdir -p "$(dirname "$slowest")"
  awk '
    function attr(s, k,   v) {
      if (!match(s, " " k "=\"[^\"]*\"")) return ""
      v = substr(s, RSTART, RLENGTH); sub(/^[^"]*"/, "", v); sub(/"$/, "", v); return v
    }
    /<testcase / { printf "%9.3fs  %s %s\n", attr($0, "time"), attr($0, "classname"), attr($0, "name") }
  ' "$junit" | sort -rn | head -n 10 > "$slowest"
  echo "ten slowest tests (also in $slowest):"
  cat "$slowest"
else
  echo "test-step: no JUnit output at $junit, so no ten slowest tests" >&2
  [ "$rc" != 0 ] || rc=1
fi
exit "$rc"
