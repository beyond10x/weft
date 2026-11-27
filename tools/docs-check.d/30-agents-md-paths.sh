#!/bin/sh
# Usage: tools/docs-check.d/30-agents-md-paths.sh [root]     (default: the repository root)
# KPI repo.agents-md-dead-paths of design:quality-kpis: repository paths named in AGENTS.md that do
# not exist. A path is the first word of an inline code span that contains `/`, outside code
# blocks and outside the section of a heading that says "Planned" (to the next heading of the
# same or a higher level). Not paths: a format name (`ess-http/1`), a package (`@scope/name`), a
# path outside the repository (`~/...`, `/...`, a URL), a template or pattern (`<dir>/x`, `*`,
# `$VAR`, `{...}`, `.../`). A `:<line>` or `#<anchor>` suffix is not part of the path.
# A path is in the tree when it is on disk and, in a git work tree whose top is the root, git
# tracks it or would add it (not ignored), as in the forge's clean checkout; in another tree (a
# fixture) when it is on disk. A path that is not in the tree is still found:
# - under the directory that opens its bullet (`scenarios/` in the bullet of `ess/` is
#   `ess/scenarios/`);
# - as the output of a tool: the bullet says "writes `<path>`" and names an executable file
#   whose text holds the path as a whole token (`tools/synth.sh` writes `target/generated/`).
#   A path a tool only reads must be in the tree.
# Prints `AGENTS.md:<line> <path>` per path, each path once, then `repo.agents-md-dead-paths
# <count>`; exits 1 when the count is above 0, 2 when it cannot read AGENTS.md. sh, awk, grep, sed
# and git only; no network.
set -uf
LC_ALL=C
export LC_ALL
root=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$root" 2> /dev/null || { echo "30-agents-md-paths: no directory $root" >&2; exit 2; }
[ -f AGENTS.md ] || { echo "30-agents-md-paths: no AGENTS.md in $root" >&2; exit 2; }

git_top=
top=$(git rev-parse --show-toplevel 2> /dev/null) && [ "$top" = "$(pwd -P)" ] && git_top=yes
# present <path>: the path is in the tree (see above).
present() {
  [ -e "$1" ] || return 1
  [ -z "$git_top" ] && return 0
  [ -n "$(git --literal-pathspecs ls-files --cached --others --exclude-standard -- "$1" | head -n 1)" ]
}

# One record per path, fields split by the unit separator (octal 037, kept when empty): <line>,
# <path>, <1 when the bullet says "writes" right before one mention of it, else 0>, <the
# directory that opens its bullet>, <the paths of the bullet, space-separated>. A bullet (or a
# paragraph) ends at a blank line, a heading, a code block or the next bullet.
records=$(awk '
  function flush(   i, written) {
    for (i = 1; i <= n; i++) if (W[i]) written[P[i]] = 1
    for (i = 1; i <= n; i++) print L[i] "\037" P[i] "\037" (P[i] in written ? 1 : 0) "\037" head "\037" named
    n = 0; head = ""; named = ""; spans = 0; carry = ""; carrying = 0; before = ""
  }
  function span(s, line, writes,   w) {
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
    n++; L[n] = line; P[n] = w; W[n] = writes
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
    if (carrying) { s = "`" carry " " $0; first = carryline; before = carrybefore; carrying = 0 }
    while ((p = index(s, "`")) > 0) {
      before = before substr(s, 1, p - 1)
      rest = substr(s, p + 1)
      q = index(rest, "`")
      if (q == 0) { carry = rest; carrying = 1; carryline = first; carrybefore = before; s = ""; break }
      span(substr(rest, 1, q - 1), first, before ~ /(^|[^A-Za-z])writes[ \t]*$/ ? 1 : 0)
      before = ""
      s = substr(rest, q + 1)
      first = NR
    }
    if (!carrying) before = before s " "
  }
  END { flush() }
' AGENTS.md) || { echo "30-agents-md-paths: awk failed" >&2; exit 2; }

# found <path> <writes> <head> <named>: exit 0 when the path is in the tree, under the head
# directory, or written by an executable of the bullet that holds it as a whole token.
found() {
  present "$1" && return 0
  [ -n "$3" ] && [ "$1" != "$3" ] && present "$3$1" && return 0
  [ "$2" = 1 ] || return 1
  token=$(printf '%s\n' "${1%/}" | sed 's/[][\\.^$*+?(){}|]/\\&/g')
  for tool in $4; do
    [ "$tool" != "$1" ] && [ -f "$tool" ] && [ -x "$tool" ] &&
      grep -qE -- "(^|[^A-Za-z0-9._/-])$token/?([^A-Za-z0-9._/-]|\$)" "$tool" && return 0
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
  line=$1 path=$2 writes=$3 head=${4:-} named=${5:-}
  IFS=' '
  found "$path" "$writes" "$head" "$named" && { IFS=$nl; continue; }
  IFS=$nl
  shown=$path
  case "$path" in
    "$head"*) ;;
    *) if [ -n "$head" ] && ! present "${path%%/*}"; then shown=$head$path; fi ;;
  esac
  case "$seen" in *" $shown "*) continue ;; esac
  seen="$seen$shown "
  echo "AGENTS.md:$line $shown"
  count=$((count + 1))
done
unset IFS

echo "repo.agents-md-dead-paths $count"
[ "$count" = 0 ]
