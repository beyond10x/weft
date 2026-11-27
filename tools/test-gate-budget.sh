#!/bin/sh
# Self-test of the test budget (~/beyond10x/AGENTS.md "Tests stay fast"), in the style of
# tools/test-planning-only.sh; task gate-budget, run by `task check`. Builds the fixture project
# tools/budget-fixture (outside the root workspace) once, then checks both sides of each limit:
#   - the passing fixture test exits 0;
#   - the sleeping test under a lowered nextest kill (profile kill) is reported TIMEOUT, exit non-zero;
#   - tools/test-step.sh with WEFT_STEP_LIMIT=3 around the sleeping test exits 124, and its
#     teardown hooks run after that killed run;
#   - tools/run-hooks.sh stops non-zero at a failing hook, before the next one;
#   - test-step.sh fails before the tests when rustc differs from the pinned channel, and writes
#     the slowest tests of a run;
#   - the real limits, read from the files: every slow-timeout of .config/nextest.toml kills at
#     most 60 s after a test starts, the default WEFT_STEP_LIMIT of tools/test-step.sh is at most
#     120, and neither Taskfile.yml nor a workflow sets WEFT_STEP_LIMIT.
# The cases end within 30 s together (the fixture build is not counted).
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
work=$root/target/gate-budget
fixture=$root/tools/budget-fixture/Cargo.toml
fixture_target=$root/target/budget-fixture
fixture_junit=$fixture_target/nextest/default/junit.xml
fail=0
pass=0
ok() { pass=$((pass + 1)); echo "ok: $*"; }
bad() { fail=1; echo "FAIL: $*"; }
hook() { printf '#!/bin/sh\n%s\n' "$2" > "$1" && chmod +x "$1"; }

# The largest period times terminate-after of any slow-timeout in a nextest config. Fails when a
# slow-timeout has no terminate-after or a period it cannot read, or [profile.default] has none.
kill_seconds() {
  awk '
    /^[ \t]*\[/ { sect = $0; gsub(/[ \t]/, "", sect); next }
    /^[ \t]*slow-timeout[ \t]*=/ {
      if (sect == "[profile.default]") seen = 1
      if (!match($0, /period[ \t]*=[ \t]*"[^"]*"/)) { bad = 1; next }
      p = substr($0, RSTART, RLENGTH); sub(/^[^"]*"/, "", p); sub(/"$/, "", p)
      if (p ~ /^[0-9]+s$/) s = p + 0
      else if (p ~ /^[0-9]+m$/) s = (p + 0) * 60
      else { bad = 1; next }
      if (!match($0, /terminate-after[ \t]*=[ \t]*[0-9]+/)) { bad = 1; next }
      t = substr($0, RSTART, RLENGTH); sub(/^[^=]*=[ \t]*/, "", t)
      if (s * t > max) max = s * t
    }
    END { if (bad || !seen) exit 1; print max }' "$1"
}

# The default of WEFT_STEP_LIMIT in a copy of tools/test-step.sh.
step_limit() {
  v=$(sed -n 's/.*WEFT_STEP_LIMIT:-\([0-9][0-9]*\)}.*/\1/p' "$1" | head -n 1)
  [ -n "$v" ] && echo "$v"
}

limits_hold() {
  k=$(kill_seconds "$1") || { echo "  no slow-timeout with a readable period and terminate-after in $1"; return 1; }
  l=$(step_limit "$2") || { echo "  no default WEFT_STEP_LIMIT in $2"; return 1; }
  echo "  kill per test ${k}s (at most 60), step limit ${l}s (at most 120)"
  [ "$k" -le 60 ] && [ "$l" -le 120 ]
}

rm -rf "$work"
mkdir -p "$work/empty.d" "$work/setup.d" "$work/teardown.d" "$work/stop.d"

# The real limits, and the same reading on raised copies.
if limits_hold .config/nextest.toml tools/test-step.sh; then ok "real limits"; else bad "real limits"; fi
cp .config/nextest.toml "$work/raised-profile.toml"
printf '\n[profile.raised]\nslow-timeout = { period = "61s", terminate-after = 1 }\n' >> "$work/raised-profile.toml"
if limits_hold "$work/raised-profile.toml" tools/test-step.sh >/dev/null; then bad "a profile killing at 61 s passed"; else ok "a profile killing at 61 s fails"; fi
sed 's/, *terminate-after *= *[0-9]*//' .config/nextest.toml > "$work/no-kill.toml"
if limits_hold "$work/no-kill.toml" tools/test-step.sh >/dev/null; then bad "a slow-timeout without terminate-after passed"; else ok "a slow-timeout without terminate-after fails"; fi
sed 's/WEFT_STEP_LIMIT:-[0-9]*}/WEFT_STEP_LIMIT:-121}/' tools/test-step.sh > "$work/raised-step.sh"
if limits_hold .config/nextest.toml "$work/raised-step.sh" >/dev/null; then bad "a step limit of 121 passed"; else ok "a step limit of 121 fails"; fi
if grep -l 'WEFT_STEP_LIMIT' Taskfile.yml .github/workflows/*.yml; then bad "the files above set WEFT_STEP_LIMIT"; else ok "no task or workflow sets WEFT_STEP_LIMIT"; fi

# Build the fixture once.
if ! cargo nextest run --manifest-path "$fixture" --target-dir "$fixture_target" --no-run > "$work/build.log" 2>&1; then
  cat "$work/build.log"
  echo "FAIL: the fixture does not build"
  exit 1
fi
start=$(date +%s)
fx() { cargo nextest run --manifest-path "$fixture" --target-dir "$fixture_target" --color never "$@"; }

fx passes_at_once > "$work/pass.log" 2>&1
rc=$?
if [ "$rc" = 0 ]; then ok "the passing test exits 0"; else cat "$work/pass.log"; bad "the passing test exited $rc"; fi

fx --profile kill sleeps_ten_seconds > "$work/kill.log" 2>&1
rc=$?
if [ "$rc" != 0 ] && grep -q 'TIMEOUT' "$work/kill.log"; then
  ok "the sleeping test under a 2 s kill is TIMEOUT, exit $rc"
else
  cat "$work/kill.log"; bad "the sleeping test under a 2 s kill: exit $rc, TIMEOUT not reported"
fi

hook "$work/setup.d/10-mark" "touch '$work/setup-ran'"
hook "$work/teardown.d/10-mark" "touch '$work/teardown-ran'"
WEFT_STEP_LIMIT=3 WEFT_SETUP_DIR=$work/setup.d WEFT_TEARDOWN_DIR=$work/teardown.d \
  WEFT_JUNIT=$fixture_junit WEFT_SLOWEST=$work/slowest-killed.txt \
  tools/test-step.sh cargo nextest run --manifest-path "$fixture" --target-dir "$fixture_target" --color never sleeps_ten_seconds \
  > "$work/step.log" 2>&1
rc=$?
if [ "$rc" = 124 ]; then ok "the step past WEFT_STEP_LIMIT=3 exits 124"; else cat "$work/step.log"; bad "the step past WEFT_STEP_LIMIT=3 exited $rc"; fi
if [ -f "$work/setup-ran" ] && [ -f "$work/teardown-ran" ]; then
  ok "setup and teardown hooks ran around the killed run"
else
  cat "$work/step.log"; bad "setup or teardown hook did not run around the killed run"
fi

WEFT_SETUP_DIR=$work/empty.d WEFT_TEARDOWN_DIR=$work/empty.d \
  WEFT_JUNIT=$fixture_junit WEFT_SLOWEST=$work/slowest.txt \
  tools/test-step.sh cargo nextest run --manifest-path "$fixture" --target-dir "$fixture_target" --color never passes_at_once \
  > "$work/slowest.log" 2>&1
rc=$?
line=$(grep 'passes_at_once' "$work/slowest.txt" 2>/dev/null | head -n 1)
if [ "$rc" = 0 ] && [ -n "$line" ] && grep -qF "$line" "$work/slowest.log" && grep -q 'rustc ' "$work/slowest.log"; then
  ok "the step prints rustc -V and the slowest tests, and writes them to its file"
else
  cat "$work/slowest.log"; bad "the step (exit $rc) did not print rustc -V and the slowest tests to its output and its file"
fi

printf '[toolchain]\nchannel = "0.0.1"\n' > "$work/other-toolchain.toml"
WEFT_TOOLCHAIN_FILE=$work/other-toolchain.toml WEFT_SETUP_DIR=$work/empty.d WEFT_TEARDOWN_DIR=$work/empty.d \
  tools/test-step.sh touch "$work/tests-ran" > "$work/rustc.log" 2>&1
rc=$?
if [ "$rc" != 0 ] && [ ! -e "$work/tests-ran" ]; then
  ok "the step fails before the tests when rustc differs from the pin, exit $rc"
else
  cat "$work/rustc.log"; bad "the step with another pinned channel exited $rc or ran the tests"
fi

printf 'not a hook\n' > "$work/stop.d/00-README.md"
hook "$work/stop.d/10-first" "touch '$work/first-ran'"
hook "$work/stop.d/20-fails" "exit 1"
hook "$work/stop.d/30-later" "touch '$work/later-ran'"
tools/run-hooks.sh "$work/stop.d" > "$work/hooks.log" 2>&1
rc=$?
if [ "$rc" != 0 ] && [ -f "$work/first-ran" ] && [ ! -e "$work/later-ran" ]; then
  ok "a failing hook stops the run before the next one, exit $rc"
else
  cat "$work/hooks.log"; bad "a failing hook: exit $rc, first ran: $([ -f "$work/first-ran" ] && echo yes || echo no), later ran: $([ -e "$work/later-ran" ] && echo yes || echo no)"
fi

took=$(($(date +%s) - start))
if [ "$took" -le 30 ]; then ok "the fixture cases took ${took}s (at most 30)"; else bad "the fixture cases took ${took}s, over 30"; fi

[ "$fail" = 0 ] && echo "gate-budget: $pass cases pass"
exit "$fail"
