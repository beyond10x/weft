#!/bin/sh
# Reads changed paths, one per line, on stdin. ~/beyond10x/AGENTS.md "Planning-only pull requests
# skip the tests": planning and prose files are .engineering/ and *.md outside the built docs site
# (website/); a change to the docs site skips the code gate and still runs the docs build.
#
#   planning-only.sh          for the gate (task check): exits 0 when every path is a planning or
#                             prose file or under website/, 1 otherwise
#   planning-only.sh --docs   for the docs build (pages.yml): exits 0 when every path is a planning
#                             or prose file, 1 otherwise; a path under website/ builds the docs
#
# A page with a generated block (a line starting `<!-- generated: ` or the line `<!-- domain-map -->`)
# is not planning-only for the gate: the gate's hooks (tools/check.d/30-docs-spec-pages.sh,
# 30-domain-map.sh) compare those blocks with ess/. The pages are found by their markers, not from
# a list, on both sides of the change: in the checked-out tree and at the base revision, so a
# change that takes the markers out of a page or deletes a marked page runs the gate too. The base
# is PLANNING_ONLY_BASE (a revision; `-` reads the head side only), else the merge base of HEAD
# with origin/main; a base that cannot be read makes the change not planning-only. An empty list
# is not planning-only.
root=$(cd "$(dirname "$0")/.." && pwd)
mode=gate
case "${1:-}" in
  "") ;;
  --docs) mode=docs ;;
  *) echo "usage: tools/planning-only.sh [--docs] < paths" >&2; exit 2 ;;
esac
marked() { grep -q -e '^<!-- generated: ' -e '^<!-- domain-map -->$'; }
base=
if [ "$mode" = gate ]; then
  base=${PLANNING_ONLY_BASE:-}
  [ -n "$base" ] || base=$(git -C "$root" merge-base HEAD origin/main 2> /dev/null) || base=
  if [ -z "$base" ]; then
    echo "not planning-only: no base revision (PLANNING_ONLY_BASE, or the merge base with origin/main)"; exit 1
  fi
  if [ "$base" != - ] && ! git -C "$root" rev-parse -q --verify "$base^{commit}" > /dev/null 2>&1; then
    echo "not planning-only: cannot read the base revision $base"; exit 1
  fi
fi
seen=0
while IFS= read -r path; do
  [ -n "$path" ] || continue
  seen=1
  if [ "$mode" = gate ]; then
    if [ -f "$root/$path" ] && marked < "$root/$path"; then
      echo "generated block: $path"; exit 1
    fi
    if [ "$base" != - ] && git -C "$root" cat-file -e "$base:$path" 2> /dev/null &&
      git -C "$root" show "$base:$path" | marked; then
      echo "generated block at the base $base: $path"; exit 1
    fi
  fi
  case "$path" in
    .engineering/*) ;;
    website/ux/fixtures/sim-player*) echo "not planning-only: $path (a weft-sim test holds it)"; exit 1 ;;
    website/*)
      if [ "$mode" = docs ]; then echo "docs site: $path"; exit 1; fi ;;
    *.md) ;;
    *) echo "not planning-only: $path"; exit 1 ;;
  esac
done
[ "$seen" = 1 ] || { echo "not planning-only: no changed paths"; exit 1; }
echo "planning-only"
exit 0
