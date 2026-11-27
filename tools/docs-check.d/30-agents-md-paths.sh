#!/bin/sh
# Usage: tools/docs-check.d/30-agents-md-paths.sh [root]     (default: the repository root)
# KPI repo.agents-md-dead-paths of design:quality-kpis: repository paths named in AGENTS.md that do
# not exist. A path is the first word of an inline code span that contains `/`, outside code
# blocks and outside the section of a heading that says "Planned" (to the next heading of the
# same or a higher level). Not paths: a format name (`ess-http/1`), a package (`@scope/name`), a
# path outside the repository (`~/...`, `/...`, a URL), a template or pattern (`<dir>/x`, `*`,
# `$VAR`, `{...}`, `.../`). A `:<line>` or `#<anchor>` suffix is not part of the path.
# A path that does not exist is still found:
# - under the directory that opens its bullet (`scenarios/` in the bullet of `ess/` is
#   `ess/scenarios/`);
# - as the output of a tool its bullet names: an executable file named in the same bullet whose
#   text contains the path (`target/generated/` beside `tools/synth.sh`).
# Prints `AGENTS.md:<line> <path>` per path, each path once, then `repo.agents-md-dead-paths
# <count>`; exits 1 when the count is above 0, 2 when it cannot read AGENTS.md. sh, awk and grep
# only; no network.
set -uf
LC_ALL=C
export LC_ALL
root=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$root" 2> /dev/null || { echo "30-agents-md-paths: no directory $root" >&2; exit 2; }
[ -f AGENTS.md ] || { echo "30-agents-md-paths: no AGENTS.md in $root" >&2; exit 2; }

# One record per path, fields split by the unit separator (octal 037, kept when empty): <line>,
# <path>, <the directory that opens its bullet>, <the paths of the bullet, space-separated>. A
# bullet (or a paragraph) ends at a blank line, a heading, a code block or the next bullet.
records=$(awk '
  function flush(   i) {
    for (i = 1; i <= n; i++) print L[i] "\037" P[i] "\037" head "\037" named
    n = 0; head = ""; named = ""; spans = 0; carry = ""; carrying = 0
  }
  function span(s, line,   w) {
    spans++
    sub(/^[ \t]+/, "", s)
    w = s
    sub(/[ \t].*/, "", w)
    if (w !~ /\//) return
    if (w ~ /^(~|\/|@|\.\.\.)/ || w ~ /:\/\// || w ~ /[<>*{}$]/) return
    if (w ~ /^[A-Za-z0-9._-]+\/[0-9]+$/) return
    sub(/#.*/, "", w)
    sub(/:[0-9]+$/, "", w)
    if (w == "") return
    if (spans == 1 && w ~ /\/$/) head = w
    named = named (named == "" ? "" : " ") w
    if (planned) return
    n++; L[n] = line; P[n] = w
  }
  /^[ \t]*(```|~~~)/ { flush(); fence = !fence; next }
  fence { next }
  /^#+[ \t]/ {
    flush()
    match($0, /^#+/); level = RLENGTH
    if (planned && level <= at) planned = 0
    if (!planned && $0 ~ /Planned/) { planned = 1; at = level }
    next
  }
  /^[ \t]*$/ { flush(); next }
  /^[ \t]*([-*+]|[0-9]+[.)])[ \t]/ { flush() }
  {
    s = $0
    first = NR
    if (carrying) { s = "`" carry " " $0; first = carryline; carrying = 0 }
    while ((p = index(s, "`")) > 0) {
      rest = substr(s, p + 1)
      q = index(rest, "`")
      if (q == 0) { carry = rest; carrying = 1; carryline = first; break }
      span(substr(rest, 1, q - 1), first)
      s = substr(rest, q + 1)
      first = NR
    }
  }
  END { flush() }
' AGENTS.md) || { echo "30-agents-md-paths: awk failed" >&2; exit 2; }

# found <path> <head> <named>: exit 0 when the path exists, under the head directory, or is named
# by an executable of the bullet.
found() {
  [ -e "$1" ] && return 0
  [ -n "$2" ] && [ "$1" != "$2" ] && [ -e "$2$1" ] && return 0
  for tool in $3; do
    [ "$tool" != "$1" ] && [ -f "$tool" ] && [ -x "$tool" ] && grep -qF -- "${1%/}" "$tool" && return 0
  done
  return 1
}

count=0
seen=' '
nl='
'
sep=$(printf '\037')
IFS=$nl
for record in $records; do
  IFS=$sep
  # shellcheck disable=SC2086 # split on the unit separator; globbing is off (set -f)
  set -- $record
  IFS=$nl
  line=$1 path=$2 head=${3:-} named=${4:-}
  IFS=' '
  found "$path" "$head" "$named" && { IFS=$nl; continue; }
  IFS=$nl
  shown=$path
  if [ -n "$head" ] && [ ! -e "${path%%/*}" ]; then shown=$head$path; fi
  case "$seen" in *" $shown "*) continue ;; esac
  seen="$seen$shown "
  echo "AGENTS.md:$line $shown"
  count=$((count + 1))
done
unset IFS

echo "repo.agents-md-dead-paths $count"
[ "$count" = 0 ]
