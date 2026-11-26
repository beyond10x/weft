#!/bin/sh
# Reads changed paths, one per line, on stdin. Exits 0 when every path is a planning or prose file
# (.engineering/ or a *.md outside a built docs site; weft builds none), 1 otherwise. An empty list
# is not planning-only. ~/beyond10x/AGENTS.md "Planning-only pull requests skip the tests".
seen=0
while IFS= read -r path; do
  [ -n "$path" ] || continue
  seen=1
  case "$path" in
    .engineering/*) ;;
    *.md) ;;
    *) echo "not planning-only: $path"; exit 1 ;;
  esac
done
[ "$seen" = 1 ] || { echo "not planning-only: no changed paths"; exit 1; }
echo "planning-only"
exit 0
