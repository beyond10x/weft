#!/bin/sh
# Self-test of the test budget (~/beyond10x/AGENTS.md "Tests stay fast"), in the style of
# tools/test-planning-only.sh; task gate-budget, run by `task check`. Builds the fixture project
# tools/budget-fixture (outside the root workspace) once, then checks both sides of each limit:
#   - the passing fixture test exits 0;
#   - the sleeping test under a lowered nextest kill (profile kill) is reported TIMEOUT, exit non-zero;
#   - tools/test-step.sh with WEFT_STEP_LIMIT=3 around the sleeping test exits 124, and its
#     teardown hooks run after that killed run;
#   - tools/run-hooks.sh stops non-zero at a failing hook, before the next one;
#   - test-step.sh fails before the tests when rustc or cargo-nextest differs from its pin, and
#     writes the slowest tests of a run;
#   - the real limits, read from the files: every slow-timeout of .config/nextest.toml kills at
#     most 60 s after a test starts (strict form, see kill_seconds), the default WEFT_STEP_LIMIT
#     of tools/test-step.sh and the most it accepts are at most 120, and neither Taskfile.yml nor
#     a workflow sets WEFT_STEP_LIMIT;
#   - the wiring of Taskfile.yml, the CI Gate job and the default command of test-step.sh;
#   - the adversary cases at the end.
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

# The largest period times terminate-after of any slow-timeout in a nextest config. Strict: every
# line that names a timeout key (slow-timeout, terminate-after, on-timeout, grace-period, in any
# form: dotted, quoted, a table header, a multi-line or commented-out part) must be exactly
#   slow-timeout = { period = "<n>s|<n>m", terminate-after = <n> }
# with nothing after it, in any section (overrides included), and [profile.default] must have one.
# So no on-timeout = "pass", no grace period past nextest's default, no kill hidden in a comment.
kill_seconds() {
  awk '
    /^[ \t]*#/ { next }
    /^[ \t]*\[/ {
      sect = $0; gsub(/[ \t]/, "", sect)
      if (sect ~ /slow-timeout|terminate-after|on-timeout|grace-period/) bad = 1
      next
    }
    /slow-timeout|terminate-after|on-timeout|grace-period/ {
      if ($0 !~ /^[ \t]*slow-timeout[ \t]*=[ \t]*\{[ \t]*period[ \t]*=[ \t]*"[0-9]+[sm]"[ \t]*,[ \t]*terminate-after[ \t]*=[ \t]*[0-9]+[ \t]*\}[ \t]*$/) { bad = 1; next }
      if (sect == "[profile.default]") seen = 1
      match($0, /"[0-9]+[sm]"/)
      p = substr($0, RSTART + 1, RLENGTH - 2)
      s = (p ~ /m$/) ? (p + 0) * 60 : p + 0
      match($0, /terminate-after[ \t]*=[ \t]*[0-9]+/)
      t = substr($0, RSTART, RLENGTH); sub(/^[^=]*=[ \t]*/, "", t)
      if (s * t > max) max = s * t
    }
    END { if (bad || !seen) exit 1; print max }' "$1"
}

# The larger of the default of WEFT_STEP_LIMIT and the most it accepts (max=) in a copy of
# tools/test-step.sh; fails when either is missing.
step_limit() {
  v=$(sed -n 's/.*WEFT_STEP_LIMIT:-\([0-9][0-9]*\)}.*/\1/p' "$1" | head -n 1)
  m=$(sed -n 's/^max=\([0-9][0-9]*\)$/\1/p' "$1" | head -n 1)
  [ -n "$v" ] && [ -n "$m" ] || return 1
  if [ "$v" -gt "$m" ]; then echo "$v"; else echo "$m"; fi
}

# The wiring: task test builds and runs tools/test-step.sh, whose default command is the root
# workspace; task check runs test, gate-budget and the check.d hooks; the CI Gate job runs task check.
wiring_holds() {
  yq -e '.tasks.test.cmds[] | select(. == "cargo nextest run --workspace --no-run")' "$1" >/dev/null 2>&1 &&
    yq -e '.tasks.test.cmds[] | select(. == "tools/test-step.sh")' "$1" >/dev/null 2>&1 &&
    yq -e '.tasks.check.cmds[] | select(.task == "test")' "$1" >/dev/null 2>&1 &&
    yq -e '.tasks.check.cmds[] | select(.task == "gate-budget")' "$1" >/dev/null 2>&1 &&
    yq -e '.tasks.check.cmds[] | select(. == "tools/run-hooks.sh tools/check.d")' "$1" >/dev/null 2>&1 &&
    yq -e '.jobs.gate.steps[] | select(.run == "task check")' "$2" >/dev/null 2>&1 &&
    grep -qx '\[ "\$#" -gt 0 \] || set -- cargo nextest run --workspace' "$3"
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
sed 's/^max=[0-9]*$/max=121/' tools/test-step.sh > "$work/raised-max.sh"
if limits_hold .config/nextest.toml "$work/raised-max.sh" >/dev/null; then bad "a step accepting WEFT_STEP_LIMIT up to 121 passed"; else ok "a step accepting WEFT_STEP_LIMIT up to 121 fails"; fi
cp .config/nextest.toml "$work/commented-kill.toml"
printf '\n[profile.commented]\nslow-timeout = { period = "30s" } # terminate-after = 2\n' >> "$work/commented-kill.toml"
if limits_hold "$work/commented-kill.toml" tools/test-step.sh >/dev/null; then bad "a kill written only in a comment passed"; else ok "a kill written only in a comment fails"; fi

# The wiring, and the same reading on broken copies.
if wiring_holds Taskfile.yml .github/workflows/gate.yml tools/test-step.sh; then ok "wiring: check runs test, test runs test-step.sh, CI runs task check"; else bad "wiring: Taskfile.yml, gate.yml or the default command of tools/test-step.sh"; fi
yq 'del(.tasks.check.cmds[] | select(.task == "test"))' Taskfile.yml > "$work/no-test-in-check.yml"
if wiring_holds "$work/no-test-in-check.yml" .github/workflows/gate.yml tools/test-step.sh; then bad "wiring: a check without task test passed"; else ok "wiring: a check without task test fails"; fi
yq '.tasks.test.cmds = ["cargo nextest run --workspace --no-run", "cargo nextest run --workspace"]' Taskfile.yml > "$work/test-without-step.yml"
if wiring_holds "$work/test-without-step.yml" .github/workflows/gate.yml tools/test-step.sh; then bad "wiring: a test task without tools/test-step.sh passed"; else ok "wiring: a test task without tools/test-step.sh fails"; fi

# The nextest pin: the step fails before the tests when cargo-nextest differs from
# nextest-version of .config/nextest.toml (the version CI installs).
mkdir -p "$work/other-nextest"
printf '#!/bin/sh\necho "cargo-nextest 0.0.1 (other)"\n' > "$work/other-nextest/cargo-nextest"
chmod +x "$work/other-nextest/cargo-nextest"
rm -f "$work/nextest-ran"
PATH=$work/other-nextest:$PATH WEFT_SETUP_DIR=$work/empty.d WEFT_TEARDOWN_DIR=$work/empty.d \
  tools/test-step.sh touch "$work/nextest-ran" > "$work/nextest-pin.log" 2>&1
rc=$?
if [ "$rc" != 0 ] && [ ! -e "$work/nextest-ran" ]; then ok "the step fails before the tests when cargo-nextest differs from the pin, exit $rc"
else cat "$work/nextest-pin.log"; bad "the step ran the tests with cargo-nextest 0.0.1, exit $rc"; fi
if grep -q '^ *nextest=$(sed .*nextest-version.* \.config/nextest\.toml' .github/workflows/gate.yml &&
  grep -qF 'tool: cargo-nextest@${{ steps.toolchain.outputs.nextest }}' .github/workflows/gate.yml; then
  ok "CI installs cargo-nextest at nextest-version of .config/nextest.toml"
else bad "CI does not install cargo-nextest at nextest-version of .config/nextest.toml"; fi

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

# Adversary cases (story:skeleton-workspace-gate): each names a way past a limit that the cases
# above let through. They run after the 30 s timing case and take about 12 s together.

# A killed test that counts as passed: nextest's on-timeout = "pass" turns TIMEOUT into
# TIMEOUT-PASS with exit 0, and kill_seconds does not read it.
sed 's/terminate-after *= *\([0-9]*\)/terminate-after = \1, on-timeout = "pass"/' .config/nextest.toml > "$work/adv-on-timeout-pass.toml"
if limits_hold "$work/adv-on-timeout-pass.toml" tools/test-step.sh >/dev/null; then
  bad "adv-on-timeout-pass: a profile whose killed tests pass (on-timeout = \"pass\") passed"
else ok "adv-on-timeout-pass: a profile whose killed tests pass fails"; fi

# A dotted-key override in the default profile: nextest applies it to every test (a 10 s test
# under period 1s, terminate-after 100 passes), the line regex of kill_seconds never sees it.
cp .config/nextest.toml "$work/adv-dotted-override.toml"
printf '\n[[profile.default.overrides]]\nfilter = "all()"\nslow-timeout.period = "30s"\nslow-timeout.terminate-after = 100\n' >> "$work/adv-dotted-override.toml"
if limits_hold "$work/adv-dotted-override.toml" tools/test-step.sh >/dev/null; then
  bad "adv-dotted-override: an override killing at 3000 s, written with dotted keys, passed"
else ok "adv-dotted-override: an override killing at 3000 s, written with dotted keys, fails"; fi

# A grace period of 600 s: a test that handles SIGTERM lives 660 s after it starts.
sed 's/terminate-after *= *\([0-9]*\)/terminate-after = \1, grace-period = "600s"/' .config/nextest.toml > "$work/adv-grace.toml"
if limits_hold "$work/adv-grace.toml" tools/test-step.sh >/dev/null; then
  bad "adv-grace-period: a profile with a 600 s grace period after the kill passed"
else ok "adv-grace-period: a profile with a 600 s grace period after the kill fails"; fi

# A step limit from the environment over 120, or 0 (GNU timeout: 0 disables the limit).
for v in 0 121; do
  rm -f "$work/adv-limit-ran"
  WEFT_STEP_LIMIT=$v WEFT_SETUP_DIR=$work/empty.d WEFT_TEARDOWN_DIR=$work/empty.d \
    WEFT_JUNIT=$work/adv-limit-junit.xml WEFT_SLOWEST=$work/adv-limit-slowest.txt \
    tools/test-step.sh touch "$work/adv-limit-ran" > "$work/adv-limit-$v.log" 2>&1
  rc=$?
  if [ "$rc" != 0 ] && [ ! -e "$work/adv-limit-ran" ]; then
    ok "adv-env-limit-$v: the step refuses WEFT_STEP_LIMIT=$v, exit $rc"
  else
    cat "$work/adv-limit-$v.log"; bad "adv-env-limit-$v: the step ran the tests under WEFT_STEP_LIMIT=$v, exit $rc"
  fi
done

# Setup hooks inside the step: a 4 s setup hook and a 0 s test under a 3 s step limit.
mkdir -p "$work/adv-slow-setup.d"
hook "$work/adv-slow-setup.d/10-sleep" "sleep 4"
WEFT_STEP_LIMIT=3 WEFT_SETUP_DIR=$work/adv-slow-setup.d WEFT_TEARDOWN_DIR=$work/empty.d \
  WEFT_JUNIT=$work/adv-setup-junit.xml WEFT_SLOWEST=$work/adv-setup-slowest.txt \
  tools/test-step.sh true > "$work/adv-setup.log" 2>&1
rc=$?
if [ "$rc" != 0 ]; then ok "adv-setup-counts: a step of 4 s under WEFT_STEP_LIMIT=3 fails, exit $rc"
else cat "$work/adv-setup.log"; bad "adv-setup-counts: a step of 4 s (setup hook) under WEFT_STEP_LIMIT=3 exited 0"; fi

# No JUnit output after the tests (a CARGO_TARGET_DIR or build.target-dir, a dropped
# [profile.default.junit]): the ten slowest tests are not printed, and the step passes.
WEFT_SETUP_DIR=$work/empty.d WEFT_TEARDOWN_DIR=$work/empty.d \
  WEFT_JUNIT=$work/adv-no-junit.xml WEFT_SLOWEST=$work/adv-no-junit-slowest.txt \
  tools/test-step.sh true > "$work/adv-no-junit.log" 2>&1
rc=$?
if [ "$rc" != 0 ]; then ok "adv-no-junit: a step that cannot print the slowest tests fails, exit $rc"
else cat "$work/adv-no-junit.log"; bad "adv-no-junit: a step without JUnit output printed no slowest tests and exited 0"; fi

# The step is ended by a signal to its process group (Ctrl-C, a cancelled job): the teardown
# hooks run, and the tests do not go on running.
mkdir -p "$work/adv-sig-teardown.d"
hook "$work/adv-sig-teardown.d/10-mark" "touch '$work/adv-sig-teardown-ran'"
setsid env WEFT_SETUP_DIR="$work/empty.d" WEFT_TEARDOWN_DIR="$work/adv-sig-teardown.d" \
  WEFT_JUNIT="$work/adv-sig-junit.xml" WEFT_SLOWEST="$work/adv-sig-slowest.txt" \
  tools/test-step.sh sh -c "sleep 3; touch '$work/adv-sig-tests-went-on'" > "$work/adv-sig.log" 2>&1 &
pg=$!
sleep 1
kill -s TERM -- "-$pg" 2>/dev/null
wait "$pg" 2>/dev/null
sleep 4
if [ -f "$work/adv-sig-teardown-ran" ]; then ok "adv-signal-teardown: the teardown hooks ran after SIGTERM"
else cat "$work/adv-sig.log"; bad "adv-signal-teardown: SIGTERM to the step's process group skipped the teardown hooks"; fi
if [ ! -e "$work/adv-sig-tests-went-on" ]; then ok "adv-signal-orphan: the tests ended with the step"
else bad "adv-signal-orphan: the tests went on running after the step was ended by SIGTERM"; fi

# The pin check reads the rustc on PATH; cargo builds with $RUSTC.
printf '#!/bin/sh\necho "rustc 0.0.1 (other)"\n' > "$work/adv-other-rustc"
chmod +x "$work/adv-other-rustc"
rm -f "$work/adv-rustc-ran"
RUSTC=$work/adv-other-rustc WEFT_SETUP_DIR=$work/empty.d WEFT_TEARDOWN_DIR=$work/empty.d \
  WEFT_JUNIT=$work/adv-rustc-junit.xml WEFT_SLOWEST=$work/adv-rustc-slowest.txt \
  tools/test-step.sh touch "$work/adv-rustc-ran" > "$work/adv-rustc.log" 2>&1
rc=$?
if [ "$rc" != 0 ] && [ ! -e "$work/adv-rustc-ran" ]; then ok "adv-rustc-env: the step fails when \$RUSTC is not the pinned channel, exit $rc"
else cat "$work/adv-rustc.log"; bad "adv-rustc-env: the step ran the tests with RUSTC=rustc 0.0.1, exit $rc"; fi

[ "$fail" = 0 ] && echo "gate-budget: $pass cases pass"
exit "$fail"
