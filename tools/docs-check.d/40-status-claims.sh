#!/bin/sh
# Usage: tools/docs-check.d/40-status-claims.sh [root]     (default: the repository root)
# KPI docs.status-claims-false of design:quality-kpis: a claim of the status page or a count the
# site states that the tree does not prove. The proofs live in website/data/status-proofs.json
# (format weft-status-proofs/1), beside website/data/status.json, whose schema (b10x-status/1)
# allows no field of its own on an item:
#   {"items":  {"<label of a status.json item>": [<proof>, ...]},
#    "counts": {"<name>": {"command": "<sh>", "nouns": ["domains", ...]}}}
# A proof is {"file": "<path>"}: the path is in the tree (on disk and, in a git work tree whose top
# is the root, tracked or new and not ignored, as in the forge's clean checkout), or
# {"command": "<sh>", "expect": "<stdout>"}: `sh -c` from the root exits 0 and, when `expect` is
# given, prints it (trailing newlines aside). A count's command prints a whole number.
# A false claim is:
# - a `shipped` item of status.json with no proof;
# - a proof that fails, or that is neither of the two forms;
# - a label of the proofs file that names no item of status.json;
# - a count whose command fails or prints no whole number;
# - a stated count, `<n> <noun>` with a noun of a count, in website/docs (.md, .mdx),
#   website/product.json or website/data/status.json that differs from the computed value. The
#   number may stand at the end of a line and its noun open the next; `**23** domains` counts;
#   a number glued to a letter, digit, `.`, `,`, `-` or `_` before it (`layer-2 domains`) or a
#   noun followed by a letter (`10 layersets`) does not.
# Commands use sh, grep, sed, awk, find and jq; no network. All of them together get 8 s, so the
# hook ends within 10 s; a command still running then is killed and fails. Prints one line per
# false claim, `<file>:<line> <item>: <reason>`, then `docs.status-claims-false <count>`; exits 1
# when the count is above 0, 2 when it cannot read a file.
set -uf
LC_ALL=C
export LC_ALL
root=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$root" 2> /dev/null || { echo "40-status-claims: no directory $root" >&2; exit 2; }
status=website/data/status.json
proofs=website/data/status-proofs.json
budget=8
for tool in jq awk timeout; do
  command -v "$tool" > /dev/null 2>&1 || { echo "40-status-claims: $tool is not on PATH" >&2; exit 2; }
done
[ -f "$status" ] || { echo "40-status-claims: no $status in $root" >&2; exit 2; }
[ -f "$proofs" ] || { echo "40-status-claims: no $proofs in $root" >&2; exit 2; }
jq -e '.items | type == "array"' "$status" > /dev/null 2>&1 ||
  { echo "40-status-claims: $status is not a b10x-status/1 document" >&2; exit 2; }
jq -e '.format == "weft-status-proofs/1" and ((.items // {}) | type == "object")
  and ((.items // {}) | all(.[]; type == "array"))
  and ((.counts // {}) | type == "object")
  and ((.counts // {}) | all(.[]; (.command | type == "string") and (.nouns | type == "array")
    and (.nouns | length > 0) and (.nouns | all(.[]; type == "string" and length > 0))))' \
  "$proofs" > /dev/null 2>&1 ||
  { echo "40-status-claims: $proofs is not a weft-status-proofs/1 document" >&2; exit 2; }

git_top=
top=$(git rev-parse --show-toplevel 2> /dev/null) && [ "$top" = "$(pwd -P)" ] && git_top=yes
# present <path>: the path is in the tree (see above).
present() {
  [ -e "$1" ] || return 1
  [ -z "$git_top" ] && return 0
  [ -n "$(git --literal-pathspecs ls-files --cached --others --exclude-standard -- "$1" | head -n 1)" ]
}

count=0
claim() { echo "$1"; count=$((count + 1)); }
json() { jq -n --arg s "$1" '$s'; }
# line_of <file> <from> <text>: the first line at or after <from> that holds <text> (passed
# through the environment: awk -v would read its backslashes as escapes).
line_of() {
  LINE_OF_TEXT=$3 awk -v from="$2" \
    'NR >= from && index($0, ENVIRON["LINE_OF_TEXT"]) { print NR; found = 1; exit } END { if (!found) print "?" }' "$1"
}

start=$(date +%s)
# run <command>: runs it from the root with what is left of the budget; sets `out` and `rc`
# (124: the budget ran out).
run() {
  left=$((budget - ($(date +%s) - start)))
  if [ "$left" -le 0 ]; then out=; rc=124; return; fi
  out=$(timeout "$left" sh -c "$1" < /dev/null 2> /dev/null)
  rc=$?
}

# A shipped item with no proof.
nl='
'
IFS=$nl
for label in $(jq -r --slurpfile p "$proofs" \
  '.items[] | select(.status == "shipped") | .label as $l
   | select((($p[0].items // {})[$l] // []) | length == 0) | $l' "$status"); do
  unset IFS
  line=$(line_of "$status" 1 "\"label\": $(json "$label")")
  claim "$status:$line $label: shipped without a proof in $proofs"
  IFS=$nl
done
unset IFS

# Every label of the proofs file in order: one that names no item, then every proof of the rest.
n=$(jq '(.items // {}) | length' "$proofs")
i=0
while [ "$i" -lt "$n" ]; do
  eval "$(jq -r --argjson i "$i" --slurpfile s "$status" \
    '(.items // {}) | to_entries[$i] | .key as $l
     | "label=\($l | @sh) known=\(if any($s[0].items[]; .label == $l) then 1 else 0 end) m=\(.value | length)"' \
    "$proofs")"
  at=$(line_of "$proofs" 1 "$(json "$label"): [")
  if [ "$known" = 0 ]; then
    claim "$proofs:$at $label: names no item of $status"
    i=$((i + 1))
    continue
  fi
  j=0
  while [ "$j" -lt "$m" ]; do
    eval "$(jq -r --argjson i "$i" --argjson j "$j" \
      '(.items // {}) | to_entries[$i].value[$j] as $p
       | (if ($p | type) == "object" and ($p | keys) == ["file"] and ($p.file | type) == "string" then "file"
          elif ($p | type) == "object" and ($p | has("command")) and ($p.command | type) == "string"
            and (($p | keys) - ["command", "expect"] | length) == 0
            and (($p.expect // "") | type) == "string" then "command"
          else "malformed" end) as $k
       | "kind=\($k | @sh) file=\(($p.file? // "") | tostring | @sh) cmd=\(($p.command? // "") | tostring | @sh)"
         + " has_expect=\(if $k == "command" and ($p | has("expect")) then 1 else 0 end)"
         + " expect=\(($p.expect? // "") | tostring | @sh) raw=\($p | tojson | @sh)"' \
      "$proofs")"
    case "$kind" in
      file)
        pl=$(line_of "$proofs" "$at" "$(json "$file")")
        present "$file" || claim "$proofs:$pl $label: file $file is not in the tree"
        ;;
      command)
        pl=$(line_of "$proofs" "$at" "$(json "$cmd" | sed 's/^"//; s/"$//')")
        run "$cmd"
        if [ "$rc" = 124 ]; then
          claim "$proofs:$pl $label: command \`$cmd\` did not end within the ${budget} s of all proofs"
        elif [ "$rc" != 0 ]; then
          claim "$proofs:$pl $label: command \`$cmd\` exited $rc"
        elif [ "$has_expect" = 1 ] && [ "$out" != "$(printf '%s\n' "$expect")" ]; then
          claim "$proofs:$pl $label: command \`$cmd\` printed $(json "$out"), want $(json "$expect")"
        fi
        ;;
      *)
        claim "$proofs:$at $label: proof $raw is neither {\"file\"} nor {\"command\", \"expect\"}"
        ;;
    esac
    j=$((j + 1))
  done
  i=$((i + 1))
done

# The counts: each command's value, then every stated count of one of its nouns.
us=$(printf '\037')
rs=$(printf '\036')
spec=
n=$(jq '(.counts // {}) | length' "$proofs")
i=0
while [ "$i" -lt "$n" ]; do
  eval "$(jq -r --argjson i "$i" \
    '(.counts // {}) | to_entries[$i] | "name=\(.key | @sh) cmd=\(.value.command | @sh) nouns=\(.value.nouns | join("\n") | @sh)"' \
    "$proofs")"
  at=$(line_of "$proofs" "$(line_of "$proofs" 1 '"counts"')" "$(json "$name"):")
  run "$cmd"
  value=
  if [ "$rc" = 124 ]; then
    claim "$proofs:$at count $name: command did not end within the ${budget} s of all proofs"
  elif [ "$rc" != 0 ]; then
    claim "$proofs:$at count $name: command exited $rc"
  else
    case "$out" in
      '' | *[!0-9]*) claim "$proofs:$at count $name: command printed $(json "$out"), not a whole number" ;;
      *) value=$out ;;
    esac
  fi
  if [ -n "$value" ]; then
    IFS=$nl
    for noun in $nouns; do spec="$spec$noun$us$value$us$name$rs"; done
    unset IFS
  fi
  i=$((i + 1))
done

if [ -n "$spec" ]; then
  files=$( { find website/docs -type f \( -name '*.md' -o -name '*.mdx' \) 2> /dev/null | sort
    for f in website/product.json "$status"; do [ -f "$f" ] && echo "$f"; done; } )
  if [ -n "$files" ]; then
    IFS=$nl
    # shellcheck disable=SC2086 # one file per line; globbing is off (set -f)
    found=$(STATUS_COUNTS=$spec awk -v us="$us" -v rs="$rs" -v proofs="$proofs" '
      BEGIN {
        spec = ENVIRON["STATUS_COUNTS"]
        k = split(spec, R, rs)
        for (r = 1; r <= k; r++) {
          if (R[r] == "") continue
          split(R[r], F, us)
          c++; noun[c] = F[1]; want[c] = F[2]; cname[c] = F[3]
        }
      }
      FNR == 1 { nf++; name[nf] = FILENAME; first[nf] = NR }
      { L[NR] = $0; file[NR] = nf }
      END {
        for (x = 1; x <= NR; x++) {
          line = L[x]; s = line; off = 0
          while (match(s, /[0-9]+/)) {
            pos = off + RSTART
            num = substr(s, RSTART, RLENGTH)
            rest = substr(s, RSTART + RLENGTH)
            off = off + RSTART + RLENGTH - 1
            s = rest
            before = pos > 1 ? substr(line, pos - 1, 1) : ""
            if (before ~ /[A-Za-z0-9.,_-]/) continue
            r = rest
            sub(/^[*_`]+/, "", r)
            if (r ~ /^[ \t]*$/ && x < NR && file[x + 1] == file[x]) r = " " L[x + 1]
            if (r !~ /^[ \t]/) continue
            sub(/^[ \t]+/, "", r)
            best = 0
            for (q = 1; q <= c; q++) {
              w = noun[q]
              if (substr(r, 1, length(w)) != w) continue
              if (substr(r, length(w) + 1, 1) ~ /[A-Za-z0-9_]/) continue
              if (!best || length(w) > length(noun[best])) best = q
            }
            if (best && num + 0 != want[best] + 0)
              print name[file[x]] ":" (x - first[file[x]] + 1) " " num " " noun[best] ": computed " want[best] " (count " cname[best] " of " proofs ")"
          }
        }
      }
    ' $files) || { unset IFS; echo "40-status-claims: awk failed" >&2; exit 2; }
    unset IFS
    if [ -n "$found" ]; then
      IFS=$nl
      for f in $found; do claim "$f"; done
      unset IFS
    fi
  fi
fi

echo "docs.status-claims-false $count"
[ "$count" = 0 ]
