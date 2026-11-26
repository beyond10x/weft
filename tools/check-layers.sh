#!/bin/sh
# Fail when a weft domain refers to a domain of a higher layer.
#
# A domain's layer is the number that prefixes its directory under ess/ (0-foundation,
# 1-infrastructure, ...). A reference is an entity field whose type names the identity type of
# another domain's entity, directly or nested in a declared struct, union, list or optional type, or a declared
# relation whose target is another domain's entity. Read from `ess specify compile` output.
#
# Usage: tools/check-layers.sh [--graph]
#   --graph  print every edge between domains with its count, then the violations.
# Replace with an `ess` command once ESS has one (beyond10x/ess story
# layers-from-the-domain-reference-graph).
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$root/.check/layers"
mkdir -p "$work"

ess specify compile --path "$root/ess" --format json > "$work/ir.json" 2> "$work/compile.err" || {
  cat "$work/compile.err" >&2
  exit 2
}

# domain -> layer, from the directory of each domain file listed in ess-inputs.yaml
: > "$work/layers.txt"
grep -E '^  - [0-9]+-[a-z-]+/[a-z-]+\.yaml$' "$root/ess/ess-inputs.yaml" | sed 's/^  - //' | while read -r path; do
  layer="${path%%-*}"
  domain="$(sed -n 's/^domain: *//p' "$root/ess/$path" | head -n 1)"
  [ -n "$domain" ] && printf '%s %s\n' "$domain" "$layer" >> "$work/layers.txt"
done

jq -r '
  (.entities | to_entries | map({key: .value.identity.type_ref.name, value: .value.domain})
    | map(select(.key != null)) | from_entries) as $owner
  # every declared type name reachable inside each declared type (struct fields, union variants,
  # list and optional elements), so an identifier nested in a value type is seen too
  | ((.types // {}) | to_entries
      | map({key: .key, value: [.value.body | .. | objects | select(.kind? == "declared") | .name]})
      | from_entries) as $direct
  | def closure:
      . as $s
      | ([$s[], ($s[] | ($direct[.] // [])[])] | unique) as $n
      | if ($n | length) == ($s | length) then $s else ($n | closure) end;
  .entities | to_entries[] | .value as $e
  | (
      ($e.fields // [])[] as $f
      | ([$f.type_ref | .. | objects | select(.kind? == "declared") | .name] | unique | closure)
      | .[] | select($owner[.] != null and $owner[.] != $e.domain)
      | "\($e.domain) \($owner[.]) \($e.name).\($f.name)"
    ),
    (
      ($e.relations // [])[]
      | select((.target | split(".")[0:2] | join(".")) != $e.domain)
      | "\($e.domain) \(.target | split(".")[0:2] | join(".")) \($e.name).\(.name)"
    )
' "$work/ir.json" > "$work/edges.raw" || { echo "check-layers: jq failed on the compiled IR" >&2; exit 2; }
sort -u "$work/edges.raw" > "$work/edges.txt"

if [ "${1:-}" = "--graph" ]; then
  awk '{print $1" -> "$2}' "$work/edges.txt" | sort | uniq -c | sort -k2,2 -k1,1nr
  echo
fi

awk '
  NR == FNR { layer[$1] = $2; next }
  {
    from = $1; to = $2
    if (!(from in layer) || !(to in layer)) { print "unknown layer: " $0; bad++; next }
    if (layer[to] + 0 > layer[from] + 0) {
      printf "upward: %s (layer %s) -> %s (layer %s) via %s\n", from, layer[from], to, layer[to], $3
      bad++
    }
  }
  END {
    if (bad) { printf "%d reference(s) point to a higher layer\n", bad; exit 1 }
    print "layers: every reference points to its own or a lower layer"
  }
' "$work/layers.txt" "$work/edges.txt"
