#!/bin/sh
# Usage: tools/docs-check.d/20-planned-banners.sh [root]     (default: the repository root)
# KPI docs.planned-without-banner of design:quality-kpis: an item of website/data/status.json with
# status `planned` whose page carries no `:::caution[Planned]` banner and does not say "Planned" in
# the section at the anchor of its href. The page of /docs/<path> is website/docs/<path>.md or .mdx,
# or <path>/index.md or .mdx. The section at #<anchor> runs from the heading whose slug (or
# `{#id}`) is <anchor> to the next heading of the same or a higher level; headings in a code
# block do not count. An item whose page is not found counts. Prints
# `website/data/status.json:<line> <label>: <href>, <reason>` per item, then
# `docs.planned-without-banner <count>`; exits 1 when the count is above 0, 2 when it cannot
# read the file. sh, jq, grep and awk only; no network.
set -uf
LC_ALL=C
export LC_ALL
root=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$root" 2> /dev/null || { echo "20-planned-banners: no directory $root" >&2; exit 2; }
status=website/data/status.json
[ -f "$status" ] || { echo "20-planned-banners: no $status in $root" >&2; exit 2; }
command -v jq > /dev/null 2>&1 || { echo "20-planned-banners: jq is not on PATH" >&2; exit 2; }
items=$(jq -r '.items[] | select(.status == "planned") | [.label, .href] | @tsv' "$status") ||
  { echo "20-planned-banners: jq cannot read $status" >&2; exit 2; }

# says_planned <file> <anchor>: exit 0 when the section at the anchor says "Planned".
says_planned() {
  awk -v want="$2" '
    function slug(t, s) {
      if (match(t, /\{#[^}]*\}[ \t]*$/)) { s = substr(t, RSTART + 2); sub(/\}[ \t]*$/, "", s); return s }
      s = tolower(t)
      gsub(/[^a-z0-9 _-]/, "", s)
      gsub(/ /, "-", s)
      return s
    }
    /^[ \t]*(```|~~~)/ { fence = !fence }
    !fence && /^#+[ \t]/ {
      match($0, /^#+/); level = RLENGTH
      t = $0
      sub(/^#+[ \t]+/, "", t)
      sub(/[ \t#]*$/, "", t)
      if (inside && level <= at) inside = 0
      if (!inside && slug(t) == want) { inside = 1; at = level }
    }
    inside && /Planned/ { found = 1 }
    END { exit !found }
  ' "$1"
}

count=0
nl='
'
tab=$(printf '\t')
IFS=$nl
for item in $items; do
  label=${item%%"$tab"*}
  href=${item#*"$tab"}
  unset IFS
  route=${href%%#*}
  anchor=
  case "$href" in *'#'*) anchor=${href#*#} ;; esac
  page=
  case "$route" in
    /docs | /docs/) base=website/docs/index ;;
    /docs/*) base=website/docs/${route#/docs/}; base=${base%/} ;;
    *) base= ;;
  esac
  if [ -n "$base" ]; then
    for candidate in "$base.md" "$base.mdx" "$base/index.md" "$base/index.mdx"; do
      [ -f "$candidate" ] && { page=$candidate; break; }
    done
  fi
  reason=
  if [ -z "$page" ]; then
    reason="no page ${base:+$base.md}"; [ -n "$base" ] || reason="no page for a route outside /docs"
  elif grep -q '^[[:space:]]*:::caution\[Planned\]' "$page"; then
    :
  elif [ -n "$anchor" ] && says_planned "$page" "$anchor"; then
    :
  elif [ -n "$anchor" ]; then
    reason="$page has no :::caution[Planned] and no Planned at #$anchor"
  else
    reason="$page has no :::caution[Planned]"
  fi
  if [ -n "$reason" ]; then
    json=$(jq -n --arg l "$label" '$l')
    line=$(grep -nF "\"label\": $json" "$status" | head -n 1 | cut -d: -f1)
    echo "$status:${line:-?} $label: $href, $reason"
    count=$((count + 1))
  fi
  IFS=$nl
done
unset IFS

echo "docs.planned-without-banner $count"
[ "$count" = 0 ]
