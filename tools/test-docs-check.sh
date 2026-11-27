#!/bin/sh
# Usage: tools/test-docs-check.sh     (task docs-check, the docs check step of pages.yml)
# Self-test of the hooks of tools/docs-check.d on the trees of tools/docs-check-fixture. For every
# <check>/pass and <check>/fail it runs the hook <check> (tools/docs-check.d/<check>.sh) with that
# tree as its argument and expects exit 0 on pass and exit 1 on fail, and a last line
# `<kpi-id> <count>` with count 0 on pass and above 0 on fail. When <check>/pass.out or
# <check>/fail.out exists, the whole output of the hook on that tree must equal it. Every
# executable hook has both trees and every fixture directory has its hook, so a new hook comes with
# its fixtures and edits neither this script nor tools/check-docs.sh. No network; ends in seconds.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/.." && pwd)
hooks=$root/tools/docs-check.d
fixtures=$root/tools/docs-check-fixture
fail=0
cases=0

no() { echo "FAIL: $*"; fail=1; }

hook_of() {
  if [ -x "$hooks/$1.sh" ]; then echo "$hooks/$1.sh"; elif [ -x "$hooks/$1" ]; then echo "$hooks/$1"; fi
}

[ -d "$fixtures" ] || { echo "test-docs-check: no directory $fixtures" >&2; exit 2; }
[ -d "$hooks" ] || { echo "test-docs-check: no directory $hooks" >&2; exit 2; }

for h in "$hooks"/*; do
  [ -f "$h" ] && [ -x "$h" ] || continue
  name=$(basename "$h" .sh)
  for side in pass fail; do
    [ -d "$fixtures/$name/$side" ] || no "hook $name has no fixture tree $name/$side"
  done
done

out=$(mktemp) || exit 2
trap 'rm -f "$out"' EXIT
for dir in "$fixtures"/*/; do
  name=$(basename "$dir")
  hook=$(hook_of "$name")
  if [ -z "$hook" ]; then no "fixture $name has no executable hook in tools/docs-check.d"; continue; fi
  for side in pass fail; do
    tree=$fixtures/$name/$side
    [ -d "$tree" ] || continue
    cases=$((cases + 1))
    "$hook" "$tree" > "$out" 2>&1
    got=$?
    want=0
    [ "$side" = fail ] && want=1
    [ "$got" = "$want" ] || { no "$name/$side: want exit $want got $got"; sed 's/^/  /' "$out"; continue; }
    last=$(tail -n 1 "$out")
    count=$(printf '%s\n' "$last" | sed -n 's/^[a-z][a-z0-9-]*\.[a-z0-9.-]* \([0-9][0-9]*\)$/\1/p')
    if [ -z "$count" ]; then
      no "$name/$side: last line is not '<kpi-id> <count>': $last"
    elif [ "$side" = pass ] && [ "$count" != 0 ]; then
      no "$name/$side: count $count, want 0"
    elif [ "$side" = fail ] && [ "$count" = 0 ]; then
      no "$name/$side: count 0, want above 0"
    fi
    if [ -f "$fixtures/$name/$side.out" ] && ! diff -u "$fixtures/$name/$side.out" "$out" > /dev/null; then
      no "$name/$side: output differs from $side.out"
      diff -u "$fixtures/$name/$side.out" "$out" | sed 's/^/  /'
    fi
  done
done

[ "$cases" -gt 0 ] || no "no case ran: no fixture tree with its hook under $fixtures"
[ "$fail" = 0 ] && echo "docs-check: $cases cases pass"
exit "$fail"
