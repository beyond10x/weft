#!/bin/sh
# Fail when a weft file refers to a higher layer, or a vendored file refers to weft's own files.
#
# Layers: a file of weft's own sits in the layer that prefixes its directory under ess/
# (0-foundation, 1-infrastructure, ...). Every vendored file, under ess/cloud/ (cloudspec's blocks,
# changed only by `task vendor-update`), sits in one layer `cloud`, below layer 0. A domain may be
# split over a vendored file and a file of weft's own (weft.workloads: ess/cloud/5-workloads/ and
# ess/4-applications/), so the unit is the part of a domain one layer declares: named by the
# domain for weft's own files (weft.workloads) and by `cloud.` and the domain without `weft.`
# for the vendored ones (cloud.workloads). Each entity belongs to the part whose file declares it.
#
# A reference is an entity field whose type names the identity type of another part's entity,
# directly or nested in a declared struct, union, list or optional type, or a declared relation
# whose target is another part's entity. An identity type more than one entity has (CloudId: the
# vendored Cloud and weft's CloudRuntime and CloudAccess) refers to the one in the lowest layer.
# A vendored file that names a type declared under ess/cloud/ refers only to a vendored entity
# with it as identity, if any: PrincipalId and InstanceId are values there (the blocks hold no
# principal and no instance), though weft's Principal and Instance have them as identity. A
# vendored file that names a type declared only in a file of weft (a value type too) refers to
# that file. Read from `ess specify compile` output and the files of ess/ess-inputs.yaml.
#
# Usage: tools/check-layers.sh [--graph]
#   --graph  print every edge between parts with its count, then the violations.
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

# Per domain file of ess-inputs.yaml (<n>-<layer>/<name>.yaml, or cloud/<n>-<layer>/<name>.yaml):
#   parts.raw     `<part> <layer>`
#   entities.txt  `<entity> <part>`, from the `- name:` items of its top-level `entities:` block
#   types.txt     `<type> <part>`, from the `- name:` items of its top-level `types:` block
: > "$work/parts.raw"
: > "$work/entities.txt"
: > "$work/types.txt"
sed -n 's#^  - \(\(cloud/\)\{0,1\}[0-9][0-9]*-[^/ ]*/[^/ ]*\.yaml\)$#\1#p' "$root/ess/ess-inputs.yaml" | while read -r path; do
  domain="$(sed -n 's/^domain: *//p' "$root/ess/$path" | head -n 1)"
  [ -n "$domain" ] || continue
  case $path in
    cloud/*) layer=cloud part="cloud.${domain#*.}" ;;
    *) layer="${path%%-*}" part="$domain" ;;
  esac
  printf '%s %s\n' "$part" "$layer" >> "$work/parts.raw"
  awk -v part="$part" -v ents="$work/entities.txt" -v types="$work/types.txt" '
    /^[A-Za-z_]+:/ { sec = $1; next }
    sec == "entities:" && /^  - name: *[^ ]+ *$/ { print $3, part >> ents }
    sec == "types:" && /^  - name: *[^ ]+ *$/ { print $3, part >> types }
  ' "$root/ess/$path"
done
sort -u "$work/parts.raw" > "$work/layers.txt"
awk '{ n[$1]++ } END { for (p in n) if (n[p] > 1) { printf "check-layers: %s is declared in more than one layer\n", p; bad = 1 } exit bad }' \
  "$work/layers.txt" >&2 || exit 2

# `O <identity type> <entity>` per entity; `T <entity> <type> <via>` per declared type a field
# names (with every declared type nested in it); `R <entity> <target entity> <via>` per relation
jq -r '
  # every declared type name reachable inside each declared type (struct fields, union variants,
  # list and optional elements), so an identifier nested in a value type is seen too
  ((.types // {}) | to_entries
      | map({key: .key, value: [.value.body | .. | objects | select(.kind? == "declared") | .name]})
      | from_entries) as $direct
  | def closure:
      . as $s
      | ([$s[], ($s[] | ($direct[.] // [])[])] | unique) as $n
      | if ($n | length) == ($s | length) then $s else ($n | closure) end;
  .entities | to_entries[] | .value as $e
  | (
      ($e.identity.type_ref.name // empty | "O \(.) \($e.name)"),
      (
        ($e.fields // [])[] as $f
        | ([$f.type_ref | .. | objects | select(.kind? == "declared") | .name] | unique | closure)
        | .[] | "T \($e.name) \(.) \($e.name).\($f.name)"
      ),
      (($e.relations // [])[] | "R \($e.name) \(.target) \($e.name).\(.name)")
    )
' "$work/ir.json" > "$work/refs.raw" || { echo "check-layers: jq failed on the compiled IR" >&2; exit 2; }

# `<from part> <to part> <via>` per reference between two parts; an entity no file declares is
# written `?<entity>` and fails below as an unknown layer
awk '
  function rank(l) { return l == "cloud" ? -1 : l + 0 }
  function partof(e) { return (e in part) ? part[e] : "?" e }
  FILENAME == ARGV[1] { layer[$1] = $2; next }
  FILENAME == ARGV[2] { part[$1] = $2; next }
  FILENAME == ARGV[3] { declared[$1] = $2; next }
  $1 == "O" { owners[$2] = owners[$2] " " $3; next }
  { ref[++n] = $0 }
  END {
    for (i = 1; i <= n; i++) {
      split(ref[i], r, " ")
      from = partof(r[2])
      fromv = (from in layer) && layer[from] == "cloud"
      if (r[1] == "R") to = partof(r[3])
      else {
        t = r[3]
        tv = (t in declared) && layer[declared[t]] == "cloud"
        # the owner in the lowest layer; a vendored file sees only the vendored owners of a type
        # declared under ess/cloud/
        to = ""
        m = (t in owners) ? split(owners[t], o, " ") : 0
        for (j = 1; j <= m; j++) {
          p = partof(o[j])
          if (fromv && tv && !((p in layer) && layer[p] == "cloud")) continue
          if (to == "" || ((p in layer) && (!(to in layer) || rank(layer[p]) < rank(layer[to]) || (rank(layer[p]) == rank(layer[to]) && p < to)))) to = p
        }
        # a vendored file that names a type declared only in a file of weft refers to that file
        if (to == "" && fromv && (t in declared) && !tv) to = declared[t]
        if (to == "") continue
      }
      if (from != to) print from, to, r[4]
    }
  }
' "$work/layers.txt" "$work/entities.txt" "$work/types.txt" "$work/refs.raw" > "$work/edges.raw" ||
  { echo "check-layers: awk failed on the references" >&2; exit 2; }
sort -u "$work/edges.raw" > "$work/edges.txt"

if [ "${1:-}" = "--graph" ]; then
  awk '{print $1" -> "$2}' "$work/edges.txt" | sort | uniq -c | sort -k2,2 -k1,1nr
  echo
fi

awk '
  function rank(l) { return l == "cloud" ? -1 : l + 0 }
  NR == FNR { layer[$1] = $2; next }
  {
    from = $1; to = $2
    if (!(from in layer) || !(to in layer)) { print "unknown layer: " $0; bad++; next }
    if (rank(layer[to]) > rank(layer[from])) {
      printf "upward: %s (layer %s) -> %s (layer %s) via %s\n", from, layer[from], to, layer[to], $3
      bad++
    }
  }
  END {
    if (bad) { printf "%d reference(s) point to a higher layer\n", bad; exit 1 }
    print "layers: every reference points to its own or a lower layer"
  }
' "$work/layers.txt" "$work/edges.txt"
