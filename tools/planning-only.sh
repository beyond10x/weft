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
# An empty list is not planning-only.
mode=gate
case "${1:-}" in
  "") ;;
  --docs) mode=docs ;;
  *) echo "usage: tools/planning-only.sh [--docs] < paths" >&2; exit 2 ;;
esac
seen=0
while IFS= read -r path; do
  [ -n "$path" ] || continue
  seen=1
  case "$path" in
    .engineering/*) ;;
    website/*)
      if [ "$mode" = docs ]; then echo "docs site: $path"; exit 1; fi ;;
    *.md) ;;
    *) echo "not planning-only: $path"; exit 1 ;;
  esac
done
[ "$seen" = 1 ] || { echo "not planning-only: no changed paths"; exit 1; }
echo "planning-only"
exit 0
