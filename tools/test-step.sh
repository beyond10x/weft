#!/bin/sh
# The test step of the gate (task test, run by `task check` locally and in CI).
# Usage: tools/test-step.sh [command...]     command default: cargo nextest run --workspace
#   1. prints `rustc -V` and fails when its version differs from the channel of the root
#      rust-toolchain.toml;
#   2. runs the setup hooks (tools/test-setup.d);
#   3. runs the tests, ended after WEFT_STEP_LIMIT seconds (default below) with exit 124;
#   4. runs the teardown hooks (tools/test-teardown.d), also after a failed or killed run;
#   5. prints the ten slowest tests, time and name, from nextest's JUnit output and writes the same
#      lines to target/nextest/slowest.txt.
# The test binaries are built before this step (cargo nextest run --workspace --no-run), so the
# limit counts test time only. tools/test-gate-budget.sh reads the default limit from this file and
# fails when it is over 120; the other variables exist for that self-test.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
limit=${WEFT_STEP_LIMIT:-120}
toolchain_file=${WEFT_TOOLCHAIN_FILE:-$root/rust-toolchain.toml}
setup_dir=${WEFT_SETUP_DIR:-$root/tools/test-setup.d}
teardown_dir=${WEFT_TEARDOWN_DIR:-$root/tools/test-teardown.d}
junit=${WEFT_JUNIT:-$root/target/nextest/default/junit.xml}
slowest=${WEFT_SLOWEST:-$root/target/nextest/slowest.txt}
[ "$#" -gt 0 ] || set -- cargo nextest run --workspace

channel=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$toolchain_file" | head -n 1)
version=$(rustc -V) || { echo "test-step: rustc -V failed" >&2; exit 1; }
echo "$version"
case $(echo "$version" | awk '{print $2}') in
  "$channel") ;;
  *) echo "test-step: rustc is not the pinned channel \"$channel\" of $toolchain_file" >&2; exit 1 ;;
esac

rm -f "$junit"
rc=0
"$root/tools/run-hooks.sh" "$setup_dir" || rc=$?
if [ "$rc" = 0 ]; then
  start=$(date +%s)
  timeout -k 10 "$limit" "$@" || rc=$?
  took=$(($(date +%s) - start))
  case $rc in
    124 | 137) if [ "$took" -ge "$limit" ]; then
        rc=124
        echo "test-step: the tests ran past the step limit of ${limit}s and were ended" >&2
      fi ;;
  esac
fi
"$root/tools/run-hooks.sh" "$teardown_dir" || { t=$?; [ "$rc" != 0 ] || rc=$t; }

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
  echo "ten slowest tests: no JUnit output at $junit"
fi
exit "$rc"
