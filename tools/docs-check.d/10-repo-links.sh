#!/bin/sh
# Usage: tools/docs-check.d/10-repo-links.sh [root]     (default: the repository root)
# KPI docs.dead-repo-links of design:quality-kpis: links of the docs site (the files of website/,
# without its build output and node_modules) to github.com/beyond10x/weft/(tree|blob)/main/<path>
# whose <path> does not exist in the tree. A fragment (#L3) or query (?plain=1), a trailing slash
# and the punctuation that ends a sentence are not part of the path. Prints `<file>:<line> <link>`
# per dead link, then `docs.dead-repo-links <count>`; exits 1 when the count is above 0, 2 when it
# cannot read the tree. sh, find, grep, sed and sort only; no network.
set -uf
LC_ALL=C
export LC_ALL
root=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$root" 2> /dev/null || { echo "10-repo-links: no directory $root" >&2; exit 2; }
[ -d website ] || { echo "10-repo-links: no website/ in $root" >&2; exit 2; }

link='github\.com/beyond10x/weft/(tree|blob)/main(/[^]["'\''()<>`|{[:space:]}]*)?'
nl='
'
files=$(find website \( -name node_modules -o -path website/build -o -path website/.docusaurus \) -prune \
  -o -type f ! -name package-lock.json -print) || { echo "10-repo-links: find failed" >&2; exit 2; }
found=
if [ -n "$files" ]; then
  IFS=$nl
  # shellcheck disable=SC2086 # one file per line; globbing is off (set -f)
  found=$(grep -HnoIE "$link" $files)
  [ "$?" -le 1 ] || { echo "10-repo-links: grep failed" >&2; exit 2; }
  unset IFS
  found=$(printf '%s\n' "$found" | sort -t: -k1,1 -k2,2n)
fi

count=0
IFS=$nl
for hit in $found; do
  at=${hit%%:github.com/*}
  url=${hit#"$at:"}
  path=$(printf '%s\n' "$url" | sed -E 's,^github\.com/beyond10x/weft/(tree|blob)/main/?,,; s/[?#].*//; s/[.,;:!]+$//; s,/+$,,')
  [ -z "$path" ] && continue
  [ -e "$path" ] && continue
  echo "$at $url"
  count=$((count + 1))
done
unset IFS

echo "docs.dead-repo-links $count"
[ "$count" = 0 ]
