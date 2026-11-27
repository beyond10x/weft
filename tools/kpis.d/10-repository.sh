#!/bin/sh
# The repository KPIs of design:quality-kpis section 4 (repo.*), measured with the commands the
# catalogue names (M1, M3, M4, M12). The command of the assessor declaration
# quality/assessors/repo-facts@1.assessor.yaml: one line per KPI on stdout (id, value with unit,
# command), the same as tab-separated lines in target/kpis/repository/values.tsv, the logs of the
# runs beside it. A KPI it cannot measure here prints `unavailable` and the reason.
#
# It measures, so it starts without the slot prefix. The runs that build or test (task check,
# tools/test-step.sh, the gate self-test) go through the pool with `slot --here`, one job each,
# so they run on this host and every value is measured under the same conditions; their time is
# taken inside the job, so it is the run time and not the wait in the queue.
# Without slot on PATH (CI) they run directly. The other counts read the tree.
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

# repo.test-step-seconds and repo.slowest-test-seconds (M4): the root tools/test-step.sh, timed on
# its own; task check above built its test binaries.
cmd="${shown}tools/test-step.sh"
slowest=target/nextest/slowest.txt
if [ ! -x tools/test-step.sh ]; then
  unavailable repo.test-step-seconds "no tools/test-step.sh" "$cmd"
  unavailable repo.slowest-test-seconds "no tools/test-step.sh" "$cmd; head -n 1 $slowest"
else
  touch "$out/test.start"
  timed test tools/test-step.sh
  read -r rc ms < "$out/test.time" || rc=unknown
  if [ "$rc" = 0 ]; then
    kpi repo.test-step-seconds "$(seconds "$ms")" s "$cmd"
  else
    unavailable repo.test-step-seconds "tools/test-step.sh exit $rc, log $out/test.log" "$cmd"
  fi
  if [ "$rc" = 0 ] && [ -n "$(find "$slowest" -newer "$out/test.start" 2> /dev/null)" ]; then
    kpi repo.slowest-test-seconds "$(awk 'NR == 1 { sub(/s$/, "", $1); print $1 }' "$slowest")" s "$cmd; head -n 1 $slowest"
  else
    unavailable repo.slowest-test-seconds "tools/test-step.sh wrote no $slowest" "$cmd; head -n 1 $slowest"
  fi
fi

# repo.gate-budget-cases-failed (M3)
cmd="${shown}tools/test-gate-budget.sh | grep -c '^FAIL:'"
if [ ! -x tools/test-gate-budget.sh ]; then
  unavailable repo.gate-budget-cases-failed "no tools/test-gate-budget.sh" "$cmd"
else
  timed gate-budget tools/test-gate-budget.sh
  read -r rc ms < "$out/gate-budget.time" || rc=unknown
  failed=$(grep -c '^FAIL:' "$out/gate-budget.log")
  if [ "$rc" = 0 ] || [ "$failed" -gt 0 ]; then
    kpi repo.gate-budget-cases-failed "$failed" cases "$cmd"
  else
    unavailable repo.gate-budget-cases-failed "exit $rc with no FAIL line, log $out/gate-budget.log" "$cmd"
  fi
fi

# repo.test-binaries-per-crate (M12): the most integration-test targets of one crate, over the
# root workspace and weft-sim. Each cargo call must succeed: a value of one workspace alone is no
# value of both.
cmd="cargo metadata --no-deps (root, tools/sim) | jq: most test targets of one crate"
meta() { cargo metadata --no-deps --offline --format-version 1 "$@" 2>> "$out/metadata.log"; }
: > "$out/metadata.log"
if ! have cargo || ! have jq; then
  unavailable repo.test-binaries-per-crate "cargo or jq is not on PATH" "$cmd"
elif ! root_meta=$(meta); then
  unavailable repo.test-binaries-per-crate "cargo metadata of the root failed, log $out/metadata.log" "$cmd"
elif ! sim_meta=$(meta --manifest-path tools/sim/Cargo.toml); then
  unavailable repo.test-binaries-per-crate "cargo metadata of tools/sim failed, log $out/metadata.log" "$cmd"
elif ! bins=$(printf '%s\n%s\n' "$root_meta" "$sim_meta" |
  jq -se '[.[].packages[] | [.targets[] | select(.kind | index("test"))] | length] | max | numbers'); then
  unavailable repo.test-binaries-per-crate "no crate in cargo metadata" "$cmd"
else
  kpi repo.test-binaries-per-crate "$bins" binaries "$cmd"
fi

# The counts below read the tree through git; a tree git cannot read (no checkout, or one git
# refuses) gives no count.
git_ok=yes
git rev-parse --is-inside-work-tree > /dev/null 2> "$out/git.log" || git_ok=

# count: the non-empty lines of stdin, each counted once.
count() { awk 'NF && !seen[$0]++' | wc -l | tr -d ' '; }

# repo.python-files (M12): tracked *.py files and tracked files whose first line is a python
# shebang (a hook has no suffix).
cmd="git ls-files '*.py'; git grep -InE '^#!.*python' at line 1 | count"
if [ -z "$git_ok" ]; then
  unavailable repo.python-files "git cannot read the tree, log $out/git.log" "$cmd"
else
  ok=yes
  py=$(git ls-files -- '*.py') || ok=
  shebang=$(git grep -InE '^#!.*python')
  [ "$?" -le 1 ] || ok=
  if [ -n "$ok" ]; then
    n=$({ printf '%s\n' "$py"; printf '%s\n' "$shebang" | awk -F: '$2 == 1 { print $1 }'; } | count)
    kpi repo.python-files "$n" files "$cmd"
  else
    unavailable repo.python-files "git ls-files or git grep failed" "$cmd"
  fi
fi

# repo.toolchain-pins-disagree (M12): rust-toolchain.toml files whose channel is not the root's.
cmd="grep channel \$(git ls-files '*rust-toolchain.toml'), against the root's"
channel() { sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -n 1; }
pin=$(channel rust-toolchain.toml 2> /dev/null)
if [ -z "$git_ok" ]; then
  unavailable repo.toolchain-pins-disagree "git cannot read the tree, log $out/git.log" "$cmd"
elif [ -z "$pin" ]; then
  unavailable repo.toolchain-pins-disagree "no channel in rust-toolchain.toml" "$cmd"
elif ! pins=$(git ls-files -- '*rust-toolchain.toml'); then
  unavailable repo.toolchain-pins-disagree "git ls-files failed" "$cmd"
else
  disagree=0
  for f in $pins; do
    [ "$(channel "$f")" = "$pin" ] || disagree=$((disagree + 1))
  done
  kpi repo.toolchain-pins-disagree "$disagree" files "$cmd"
fi

# repo.cache-bypass (M12): a setting of the shared compile cache's server (SCCACHE_SERVER_PORT,
# SCCACHE_DIR, SCCACHE_CACHE_SIZE), or an empty compiler wrapper: RUSTC_WRAPPER or
# CARGO_BUILD_RUSTC_WRAPPER set to "", '' or nothing in a shell line, YAML or TOML, and
# rustc-wrapper = "" in a cargo config. The planning store's prose, this file and the fixtures of
# its test (tools/sim/tests/ratchet) name the patterns.
pattern="RUSTC_WRAPPER(=(\"\"|''|[[:space:]]|[;&|)]|\$)|:[[:space:]]*(\"\"|''|#|\$)|[[:space:]]+=[[:space:]]*(\"\"|''))"
pattern="$pattern|rustc-wrapper[[:space:]]*=[[:space:]]*(\"\"|'')|SCCACHE_(SERVER_PORT|DIR|CACHE_SIZE)"
cmd="git grep -nE '$pattern' -- . ':!.engineering' ':!tools/kpis.d/10-repository.sh' ':!tools/sim/tests/ratchet' | count"
if [ -z "$git_ok" ]; then
  unavailable repo.cache-bypass "git cannot read the tree, log $out/git.log" "$cmd"
else
  found=$(git grep -nE "$pattern" -- . ':!.engineering' ':!tools/kpis.d/10-repository.sh' ':!tools/sim/tests/ratchet')
  if [ "$?" -le 1 ]; then
    printf '%s\n' "$found" > "$out/cache-bypass.txt"
    kpi repo.cache-bypass "$(printf '%s\n' "$found" | count)" lines "$cmd"
  else
    unavailable repo.cache-bypass "git grep failed" "$cmd"
  fi
fi

# repo.plan-validate-defects: the length of the `problems` list. Without that list, or with a
# failed run that lists no problem, there is no count.
cmd="aep plan artifact validate --format json | jq '.problems | length'"
if ! have aep || ! have jq; then
  unavailable repo.plan-validate-defects "aep or jq is not on PATH" "$cmd"
else
  json=$(aep plan artifact validate --format json 2> "$out/plan-validate.log")
  rc=$?
  if defects=$(printf '%s\n' "$json" | jq -e '.problems | arrays | length' 2>> "$out/plan-validate.log") &&
    { [ "$rc" = 0 ] || [ "$defects" -gt 0 ]; }; then
    kpi repo.plan-validate-defects "$defects" defects "$cmd"
  else
    unavailable repo.plan-validate-defects "aep exit $rc with no problems list, log $out/plan-validate.log" "$cmd"
  fi
fi
exit 0
