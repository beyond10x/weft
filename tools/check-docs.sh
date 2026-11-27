#!/bin/sh
# Usage: tools/check-docs.sh     (task docs-check, the docs check step of pages.yml)
# Holds the docs claims KPIs at 0: runs every executable hook of tools/docs-check.d in name order
# on the repository root. Each prints its offending items and `<kpi-id> <count>`. Every hook
# runs, so every KPI line is printed; then the script exits with the highest exit of a hook (1: a
# count above 0, 2: a hook could not measure), 0 when all passed. Contract:
# tools/docs-check.d/README.md. DOCS_CHECK_HOOKS names another hook directory (the self-test).
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
dir=${DOCS_CHECK_HOOKS:-tools/docs-check.d}
[ -d "$dir" ] || { echo "check-docs: no directory $dir" >&2; exit 2; }
rc=0
for hook in "$dir"/*; do
  [ -f "$hook" ] && [ -x "$hook" ] || continue
  echo "check-docs: $hook"
  "$hook"
  got=$?
  if [ "$got" != 0 ]; then
    echo "check-docs: $hook failed with exit $got" >&2
    [ "$got" -gt "$rc" ] && rc=$got
  fi
done
exit "$rc"
