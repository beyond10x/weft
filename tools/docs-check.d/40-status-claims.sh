#!/bin/sh
# Usage: tools/docs-check.d/40-status-claims.sh [root]     (default: the repository root)
# KPI docs.status-claims-false of design:quality-kpis: a status claim or a count the site states
# that the tree does not prove. The proofs live in website/data/status-proofs.json (format
# weft-status-proofs/1), beside website/data/status.json, whose schema (b10x-status/1) allows no
# field of its own on an item:
#   {"items":    {"<label of a status.json item>": [<proof>, ...]},
#    "features": {"<title of a website/product.json feature>": [<proof>, ...]},
#    "counts":   {"<name>": {"command": "<sh>", "nouns": ["domains", ...]}}}
# A proof is {"file": "<path>"}: a regular, non-empty file, which git lists (tracked, or new and not
# ignored) when the root is the top of a git work tree, as in the forge's clean checkout; or
# {"command": "<sh>", "expect": "<stdout>"}: `sh -c` from the root exits 0 and, when `expect` is
# given, prints it (trailing newlines aside). A count's command prints a whole number.
# A false claim is:
# - a `shipped` item of status.json, or a `shipped` feature of product.json (an object with a
#   `status` and a `title` or `label`), with no proof;
# - a proof that fails, or that is neither of the two forms;
# - a label of the proofs file that names no item (no feature), or that it lists twice; a label
#   that status.json or a title that product.json lists twice; a count named twice;
# - a count whose command fails or prints no whole number;
# - a stated count in website/docs (.md, .mdx), website/product.json or website/data/status.json
#   that differs from the computed value: `<n> <noun>` or `<n> <word> <noun>` with a noun of a
#   count, compared without case, where <n> is digits or a number word from one to twenty-nine
#   (`twenty-four`, `twenty four`); the words may run onto the next line; `**23** domains` counts;
#   and a table row whose cell is a noun and whose next cell is a number (`| Domains | 24 |`).
#   A number glued to a letter, digit, `.`, `,`, `-` or `_` before it (`layer-2 domains`) or a
#   noun followed by a letter (`10 layersets`) is no count.
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
product=website/product.json
budget=8
for tool in jq awk timeout; do
  command -v "$tool" > /dev/null 2>&1 || { echo "40-status-claims: $tool is not on PATH" >&2; exit 2; }
done
[ -f "$status" ] || { echo "40-status-claims: no $status in $root" >&2; exit 2; }
[ -f "$proofs" ] || { echo "40-status-claims: no $proofs in $root" >&2; exit 2; }
jq -e '.items | type == "array"' "$status" > /dev/null 2>&1 ||
  { echo "40-status-claims: $status is not a b10x-status/1 document" >&2; exit 2; }
jq -e '.format == "weft-status-proofs/1"
  and all(((.items // {}), (.features // {})); type == "object" and all(.[]; type == "array"))
  and ((.counts // {}) | type == "object")
  and ((.counts // {}) | all(.[]; (.command | type == "string") and (.nouns | type == "array")
    and (.nouns | length > 0) and (.nouns | all(.[]; type == "string" and length > 0))))' \
  "$proofs" > /dev/null 2>&1 ||
  { echo "40-status-claims: $proofs is not a weft-status-proofs/1 document" >&2; exit 2; }
if [ -f "$product" ]; then
  jq -e . "$product" > /dev/null 2>&1 || { echo "40-status-claims: jq cannot read $product" >&2; exit 2; }
  productdoc=$product
else
  productdoc=/dev/null
fi

git_top=
top=$(git rev-parse --show-toplevel 2> /dev/null) && [ "$top" = "$(pwd -P)" ] && git_top=yes
# proved_file <path>: a regular, non-empty file in the tree (see above).
proved_file() {
  [ -f "$1" ] && [ -s "$1" ] || return 1
  [ -z "$git_top" ] && return 0
  [ -n "$(git --literal-pathspecs ls-files --cached --others --exclude-standard -- "$1" | head -n 1)" ]
}

count=0
claim() { printf '%s\n' "$1"; count=$((count + 1)); }
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

nl='
'
tab=$(printf '\t')

# The features of product.json: every object with a status and a title (or label).
features='[.. | objects | select(has("status") and ((.title // .label) | type) == "string")]'

# A label status.json lists twice, a title product.json lists twice.
IFS=$nl
for label in $(jq -r '[.items[].label] | group_by(.) | map(select(length > 1) | .[0]) | .[]' "$status"); do
  unset IFS
  first=$(line_of "$status" 1 "\"label\": $(json "$label")")
  claim "$status:$(line_of "$status" $((first + 1)) "\"label\": $(json "$label")") $label: listed twice in $status"
  IFS=$nl
done
for title in $(jq -r "$features | map(.title // .label) | group_by(.) | map(select(length > 1) | .[0]) | .[]" "$productdoc"); do
  unset IFS
  claim "$product:$(line_of "$product" 1 "$(json "$title")") $title: listed twice in $product"
  IFS=$nl
done
unset IFS

# A label or a count the proofs file lists twice (jq keeps only the last). In the stream every
# value of a section ends once: a closing event at depth 3 for a non-empty array or object, a
# leaf at depth 2 for an empty one or a scalar.
dups=$(jq -r --stream 'select(((length == 1 and (.[0] | length) == 3) or (length == 2 and (.[0] | length) == 2))
    and (.[0][0] == "items" or .[0][0] == "features" or .[0][0] == "counts"))
  | "\(.[0][0])\t\(.[0][1])"' "$proofs" | awk 'seen[$0]++ == 1')
IFS=$nl
for d in $dups; do
  unset IFS
  section=${d%%"$tab"*}
  label=${d#*"$tab"}
  at=$(line_of "$proofs" 1 "\"$section\": {")
  first=$(line_of "$proofs" "$at" "$(json "$label"):")
  claim "$proofs:$(line_of "$proofs" $((first + 1)) "$(json "$label"):") $label: listed twice in $section of $proofs"
  IFS=$nl
done
unset IFS

# A shipped item or feature with no proof.
IFS=$nl
for label in $(jq -r --slurpfile p "$proofs" \
  '.items[] | select(.status == "shipped") | .label as $l
   | select((($p[0].items // {})[$l] // []) | length == 0) | $l' "$status"); do
  unset IFS
  claim "$status:$(line_of "$status" 1 "\"label\": $(json "$label")") $label: shipped without a proof in $proofs"
  IFS=$nl
done
for title in $(jq -r --slurpfile p "$proofs" \
  "$features | .[] | select(.status == \"shipped\") | (.title // .label) as \$t
   | select(((\$p[0].features // {})[\$t] // []) | length == 0) | \$t" "$productdoc"); do
  unset IFS
  claim "$product:$(line_of "$product" 1 "$(json "$title")") $title: shipped without a proof in $proofs"
  IFS=$nl
done
unset IFS

# check_proofs <section> <file whose labels it names> <what a label names>: every label of the
# section in order; one that names nothing, then every proof of the rest.
check_proofs() {
  section=$1
  if [ "$section" = items ]; then
    names=$(jq -c '[.items[].label]' "$status")
  else
    names=$(jq -c "$features | map(.title // .label)" "$productdoc")
    names=${names:-[]}
  fi
  sat=$(line_of "$proofs" 1 "\"$section\": {")
  n=$(jq --arg s "$section" '(.[$s] // {}) | length' "$proofs")
  i=0
  while [ "$i" -lt "$n" ]; do
    eval "$(jq -r --arg s "$section" --argjson i "$i" --argjson names "$names" \
      '(.[$s] // {}) | to_entries[$i] | .key as $l
       | "label=\($l | @sh) known=\(if ($names | index([$l])) then 1 else 0 end) m=\(.value | length)"' \
      "$proofs")"
    at=$(line_of "$proofs" "$sat" "$(json "$label"): [")
    if [ "$known" = 0 ]; then
      claim "$proofs:$at $label: names no $3 of $2"
      i=$((i + 1))
      continue
    fi
    j=0
    while [ "$j" -lt "$m" ]; do
      eval "$(jq -r --arg s "$section" --argjson i "$i" --argjson j "$j" \
        '(.[$s] // {}) | to_entries[$i].value[$j] as $p
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
          proved_file "$file" || claim "$proofs:$pl $label: file $file is not a non-empty file in the tree"
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
}
check_proofs items "$status" item
check_proofs features "$product" feature

# The counts: each command's value, then every stated count of one of its nouns.
us=$(printf '\037')
rs=$(printf '\036')
spec=
n=$(jq '(.counts // {}) | length' "$proofs")
cat_=$(line_of "$proofs" 1 '"counts": {')
i=0
while [ "$i" -lt "$n" ]; do
  eval "$(jq -r --argjson i "$i" \
    '(.counts // {}) | to_entries[$i] | "name=\(.key | @sh) cmd=\(.value.command | @sh) nouns=\(.value.nouns | join("\n") | @sh)"' \
    "$proofs")"
  at=$(line_of "$proofs" "$cat_" "$(json "$name"):")
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
    for f in "$product" "$status"; do [ -f "$f" ] && printf '%s\n' "$f"; done; } )
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
          c++; noun[c] = tolower(F[1]); want[c] = F[2] + 0; cname[c] = F[3]
        }
        split("zero one two three four five six seven eight nine ten eleven twelve thirteen " \
          "fourteen fifteen sixteen seventeen eighteen nineteen twenty", W, " ")
        for (r = 1; r <= 21; r++) NUM[W[r]] = r - 1
      }
      # value of a number: digits, or a word from zero to twenty-nine ("twenty-four",
      # "twenty four"); -1 when it is none. Sets USED to the length it took of t.
      function value(t,   w, u) {
        if (match(t, /^[0-9]+/)) { USED = RLENGTH; return substr(t, 1, RLENGTH) + 0 }
        if (!match(t, /^[A-Za-z]+/)) return -1
        USED = RLENGTH
        w = tolower(substr(t, 1, RLENGTH))
        if (!(w in NUM)) return -1
        if (NUM[w] == 20 && match(substr(t, USED + 1), /^[- ][A-Za-z]+/)) {
          u = tolower(substr(t, USED + 2, RLENGTH - 1))
          if ((u in NUM) && NUM[u] >= 1 && NUM[u] <= 9) { USED += RLENGTH; return 20 + NUM[u] }
        }
        return NUM[w]
      }
      # the longest noun that opens t, followed by no letter or digit; 0 when none.
      function noun_at(t,   q, best, w) {
        best = 0
        t = tolower(t)
        for (q = 1; q <= c; q++) {
          w = noun[q]
          if (substr(t, 1, length(w)) != w) continue
          if (substr(t, length(w) + 1, 1) ~ /[a-z0-9_]/) continue
          if (!best || length(w) > length(noun[best])) best = q
        }
        return best
      }
      function trim(t) { gsub(/^[ \t*_`]+|[ \t*_`]+$/, "", t); return t }
      function report(x, said, q) {
        print name[file[x]] ":" (x - first[file[x]] + 1) " " said ": computed " want[q] " (count " cname[q] " of " proofs ")"
      }
      FNR == 1 { nf++; name[nf] = FILENAME; first[nf] = NR }
      { L[NR] = $0; file[NR] = nf }
      END {
        for (x = 1; x <= NR; x++) {
          line = L[x]
          T = line
          if (x < NR && file[x + 1] == file[x]) T = line " " L[x + 1]
          max = length(line)
          s = T; off = 0; skip = 0
          while (match(s, /[A-Za-z0-9]+/)) {
            pos = off + RSTART
            s = substr(s, RSTART + RLENGTH)
            off = pos + RLENGTH - 1
            if (pos > max) break
            if (pos <= skip) continue
            before = pos > 1 ? substr(T, pos - 1, 1) : ""
            if (before ~ /[A-Za-z0-9.,_-]/) continue
            v = value(substr(T, pos))
            if (v < 0) continue
            skip = pos + USED - 1
            said = substr(T, pos, USED)
            r = substr(T, pos + USED)
            if (r ~ /^[A-Za-z0-9_]/) continue
            sub(/^[*_`]+/, "", r)
            if (r !~ /^[ \t]/) continue
            sub(/^[ \t]+/, "", r)
            q = noun_at(r)
            gap = ""
            if (!q && match(r, /^[A-Za-z][A-Za-z-]*[ \t]+/)) {
              gap = substr(r, 1, RLENGTH)
              q = noun_at(substr(r, RLENGTH + 1))
            }
            if (q && v != want[q]) {
              stated = substr(r, length(gap) + 1, length(noun[q]))
              sub(/[ \t]+$/, " ", gap)
              report(x, said " " gap stated, q)
            }
          }
          # A noun-first table row: | Domains | 24 |
          if (line ~ /^[ \t]*\|/) {
            m = split(line, C, "|")
            for (i = 2; i < m; i++) {
              cell = trim(C[i])
              q = noun_at(cell)
              if (!q || length(cell) != length(noun[q])) continue
              num = trim(C[i + 1])
              v = value(num)
              if (v < 0 || USED != length(num)) continue
              if (v != want[q]) report(x, cell " | " num, q)
            }
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

printf '%s\n' "docs.status-claims-false $count"
[ "$count" = 0 ]
