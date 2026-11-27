#!/bin/sh
# The repository KPIs of design:quality-kpis section 4 (repo.*), measured with the commands the
# catalogue names (M1, M3, M4, M12). The command of the assessor declaration
# quality/assessors/repo-facts@1.assessor.yaml: one line per KPI on stdout (id, value with unit,
# command), the same as tab-separated lines in target/kpis/repository/values.tsv, the logs of the
# runs beside it. A KPI it cannot measure here prints `unavailable` and the reason.
#
# It measures, so it starts without the slot prefix. The runs that build or test (task check, task
# test, the gate self-test) go through the pool with `slot --here`, one job each, so they run on
# this host and every value is measured under the same conditions; their time is taken inside the
# job, so it is the run time and not the wait in the queue. Without slot on PATH (CI) they run
# directly. The other counts read the tree.
set -u
out=target/kpis/repository
mkdir -p "$out"
: > "$out/values.tsv"

pool=
command -v slot > /dev/null 2>&1 && pool='slot --here'
shown=${pool:+slot }

# kpi <id> <value> <unit> <command>
kpi() {
  printf '%-30s %-14s %s\n' "$1" "$2 $3" "$4"
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >> "$out/values.tsv"
}

# unavailable <id> <reason> <command>
unavailable() {
  printf '%-30s %-14s %s (%s)\n' "$1" unavailable "$3" "$2"
  printf '%s\tunavailable\t\t%s (%s)\n' "$1" "$3" "$2" >> "$out/values.tsv"
}

# timed <name> <command...>: the command in the pool, its output in $out/<name>.log, and
# "<exit> <milliseconds>" in $out/<name>.time, timed inside the job.
timed() {
  name=$1
  shift
  rm -f "$out/$name.time"
  # shellcheck disable=SC2086 # $pool is the prefix and its option, or nothing
  $pool sh -c 's=$(date +%s%N); "$@" > "$0.log" 2>&1; rc=$?; e=$(date +%s%N)
    echo "$rc $(((e - s) / 1000000))" > "$0.time"' "$out/$name" "$@"
}

seconds() { awk -v ms="$1" 'BEGIN { printf "%.1f", ms / 1000 }'; }

have() { command -v "$1" > /dev/null 2>&1; }

# repo.gate-seconds (M1)
cmd="${shown}task check"
if ! have task; then
  unavailable repo.gate-seconds "task is not on PATH" "$cmd"
else
  timed check task check
  read -r rc ms < "$out/check.time" || rc=unknown
  if [ "$rc" = 0 ]; then
    kpi repo.gate-seconds "$(seconds "$ms")" s "$cmd"
  else
    unavailable repo.gate-seconds "task check exit $rc, log $out/check.log" "$cmd"
  fi
fi

# repo.test-step-seconds and repo.slowest-test-seconds (M4)
cmd="${shown}task test"
slowest=target/nextest/slowest.txt
if ! have task; then
  unavailable repo.test-step-seconds "task is not on PATH" "$cmd"
  unavailable repo.slowest-test-seconds "task is not on PATH" "$cmd; head -n 1 $slowest"
else
  touch "$out/test.start"
  timed test task test
  read -r rc ms < "$out/test.time" || rc=unknown
  if [ "$rc" = 0 ]; then
    kpi repo.test-step-seconds "$(seconds "$ms")" s "$cmd"
  else
    unavailable repo.test-step-seconds "task test exit $rc, log $out/test.log" "$cmd"
  fi
  if [ "$rc" = 0 ] && [ -n "$(find "$slowest" -newer "$out/test.start" 2> /dev/null)" ]; then
    kpi repo.slowest-test-seconds "$(awk 'NR == 1 { sub(/s$/, "", $1); print $1 }' "$slowest")" s "$cmd; head -n 1 $slowest"
  else
    unavailable repo.slowest-test-seconds "task test wrote no $slowest" "$cmd; head -n 1 $slowest"
  fi
fi

# repo.gate-budget-cases-failed (M3)
cmd="${shown}tools/test-gate-budget.sh | grep -c '^FAIL:'"
timed gate-budget tools/test-gate-budget.sh
read -r rc ms < "$out/gate-budget.time" || rc=unknown
failed=$(grep -c '^FAIL:' "$out/gate-budget.log")
if [ "$rc" = 0 ] || [ "$failed" -gt 0 ]; then
  kpi repo.gate-budget-cases-failed "$failed" cases "$cmd"
else
  unavailable repo.gate-budget-cases-failed "exit $rc with no FAIL line, log $out/gate-budget.log" "$cmd"
fi

# repo.test-binaries-per-crate (M12): the most integration-test targets of one crate, over the
# root workspace and weft-sim.
cmd="cargo metadata --no-deps (root, tools/sim) | jq: most test targets of one crate"
if ! have cargo || ! have jq; then
  unavailable repo.test-binaries-per-crate "cargo or jq is not on PATH" "$cmd"
elif bins=$({ cargo metadata --no-deps --offline --format-version 1 &&
  cargo metadata --no-deps --offline --format-version 1 --manifest-path tools/sim/Cargo.toml; } |
  jq -s '[.[].packages[] | [.targets[] | select(.kind | index("test"))] | length] | max'); then
  kpi repo.test-binaries-per-crate "$bins" binaries "$cmd"
else
  unavailable repo.test-binaries-per-crate "cargo metadata failed" "$cmd"
fi

# repo.python-files (M12)
cmd="git ls-files '*.py' | wc -l"
kpi repo.python-files "$(git ls-files '*.py' | wc -l | tr -d ' ')" files "$cmd"

# repo.toolchain-pins-disagree (M12): rust-toolchain.toml files whose channel is not the root's.
cmd="grep channel \$(git ls-files '*rust-toolchain.toml'), against the root's"
channel() { sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -n 1; }
pin=$(channel rust-toolchain.toml)
disagree=0
for f in $(git ls-files '*rust-toolchain.toml'); do
  [ "$(channel "$f")" = "$pin" ] || disagree=$((disagree + 1))
done
kpi repo.toolchain-pins-disagree "$disagree" files "$cmd"

# repo.cache-bypass (M12): the planning store's prose and this file name the patterns.
pattern='RUSTC_WRAPPER=""|SCCACHE_SERVER_PORT|SCCACHE_DIR'
cmd="git grep -nE '$pattern' -- . ':!.engineering' ':!tools/kpis.d/10-repository.sh' | wc -l"
lines=$(git grep -nE "$pattern" -- . ':!.engineering' ':!tools/kpis.d/10-repository.sh' | wc -l | tr -d ' ')
kpi repo.cache-bypass "$lines" lines "$cmd"

# repo.plan-validate-defects
cmd="aep plan artifact validate --format json | jq '.problems | length'"
if ! have aep || ! have jq; then
  unavailable repo.plan-validate-defects "aep or jq is not on PATH" "$cmd"
elif defects=$(aep plan artifact validate --format json 2> "$out/plan-validate.log" | jq -e '.problems | length'); then
  kpi repo.plan-validate-defects "$defects" defects "$cmd"
else
  unavailable repo.plan-validate-defects "no JSON result, log $out/plan-validate.log" "$cmd"
fi
exit 0
