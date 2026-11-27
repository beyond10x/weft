#!/bin/sh
# Usage: tools/run-hooks.sh <dir>
# Runs every executable file of <dir> in name order (C collation) from the current directory, and
# stops at the first that fails with its exit code. Files that are not executable (a README.md)
# are skipped. Hook directories: tools/test-setup.d, tools/test-teardown.d, tools/check.d,
# tools/slow.d; each README.md names its contract.
set -u
LC_ALL=C
export LC_ALL
dir=${1:?usage: tools/run-hooks.sh <dir>}
[ -d "$dir" ] || { echo "run-hooks: no directory $dir" >&2; exit 2; }
for hook in "$dir"/*; do
  [ -f "$hook" ] && [ -x "$hook" ] || continue
  echo "run-hooks: $hook"
  "$hook" || { rc=$?; echo "run-hooks: $hook failed with exit $rc" >&2; exit "$rc"; }
done
exit 0
