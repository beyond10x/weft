#!/bin/sh
# Check hook: the domain map of ess/ on the docs site, the generated block between
# `<!-- domain-map -->` and `<!-- /domain-map -->` of the page DOMAIN_MAP_PAGE (default
# website/docs/specification/layers.md). Fails (exit 1) unless
#   0. the page has one opening and one closing marker, in that order, neither inside a code
#      fence (tools/gen-domain-map.sh --markers);
#   1. the block is byte for byte the one tools/gen-domain-map.sh makes from ess/ now
#      (written to target/domain-map/block.md);
#   2. every domain of `ess specify compile` (`.domains | keys[]`, not the generator's list) is
#      declared by a file, and each of its parts is exactly one node line of the block, inside
#      the `subgraph L<n>` of its layer: the number prefix of the directory of the file under
#      ess/ whose `domain:` names it, or `Lcloud` and the node `cloud.<name>` for a vendored file
#      under ess/cloud/ (weft.workloads has two parts: weft.workloads and cloud.workloads);
#   3. every pair `tools/check-layers.sh --graph` prints is drawn: an edge `<from> --> <to>`
#      between the two nodes, or `L<n> --> L<m>` between their two layers.
# Exit 2 when it cannot measure. Reads the committed page; runs `ess` (no network, about 2 s).
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 2
page=${DOMAIN_MAP_PAGE:-website/docs/specification/layers.md}
work=target/domain-map
mkdir -p "$work" || exit 2
[ -f "$page" ] || { echo "30-domain-map: no page $page" >&2; exit 2; }

# one opening and one closing marker, in that order, neither inside a code fence
DOMAIN_MAP_PAGE=$page tools/gen-domain-map.sh --markers
case $? in
  0) ;;
  1) echo "30-domain-map: $page fails"; exit 1 ;;
  *) echo "30-domain-map: tools/gen-domain-map.sh --markers failed" >&2; exit 2 ;;
esac
awk '/^<!-- domain-map -->$/ { on = 1 } on { print } /^<!-- \/domain-map -->$/ { on = 0 }' \
  "$page" > "$work/committed.md" || exit 2

tools/check-layers.sh --graph > "$work/graph.txt" || { echo "30-domain-map: tools/check-layers.sh --graph failed" >&2; exit 2; }
DOMAIN_MAP_GRAPH="$work/graph.txt" tools/gen-domain-map.sh --block > "$work/block.md" ||
  { echo "30-domain-map: tools/gen-domain-map.sh --block failed" >&2; exit 2; }
ess specify compile --path ess --format json > "$work/ir.json" 2> "$work/compile.err" ||
  { cat "$work/compile.err" >&2; exit 2; }
jq -r '.domains | keys[]' "$work/ir.json" > "$work/domains.txt" || exit 2
[ -s "$work/domains.txt" ] || { echo "30-domain-map: ess specify compile names no domain" >&2; exit 2; }

# `<part> <layer> <domain>` per file that declares a domain: the layer from its directory, or
# `cloud` for the vendored files under ess/cloud/, whose part is `cloud.<name>`
: > "$work/layers.txt"
for f in ess/[0-9]*-*/*.yaml ess/cloud/[0-9]*-*/*.yaml; do
  [ -f "$f" ] || continue
  d=$(sed -n 's/^domain:[ ]*//p' "$f" | head -n 1)
  [ -n "$d" ] || continue
  case $f in
    ess/cloud/*) printf '%s %s %s\n' "cloud.${d#*.}" cloud "$d" ;;
    *) dir=${f#ess/}; printf '%s %s %s\n' "$d" "${dir%%-*}" "$d" ;;
  esac >> "$work/layers.txt"
done

# the pairs of the graph: `<count> <from> -> <to>` lines, until the blank line
awk 'NF == 0 { exit } NF == 4 && $3 == "->" { print $2, $4 }' "$work/graph.txt" > "$work/pairs.txt"
[ -s "$work/pairs.txt" ] || { echo "30-domain-map: tools/check-layers.sh --graph printed no pair" >&2; exit 2; }

bad=0
if ! cmp -s "$work/block.md" "$work/committed.md"; then
  echo "30-domain-map: the block of $page differs from tools/gen-domain-map.sh --block; run tools/gen-domain-map.sh"
  diff "$work/committed.md" "$work/block.md" | head -n 20
  bad=1
fi

# Nodes and edges of the committed block. A node line is one token, `<id>` or `<id>[<label>]`;
# its subgraph is the innermost open `subgraph <id>`.
awk '
  FILENAME == ARGV[1] { layer[$1] = $2; parts[$3] = parts[$3] " " $1; next }
  FILENAME == ARGV[2] { want[$1] = 1; next }
  FILENAME == ARGV[3] { pair[$1 " " $2] = 1; next }
  {
    line = $0
    sub(/^[ \t]+/, "", line); sub(/[ \t]+$/, "", line)
    if (line ~ /^subgraph[ \t]/) {
      s = line; sub(/^subgraph[ \t]+/, "", s); sub(/[[ \t].*$/, "", s)
      stack[++depth] = s; next
    }
    if (line == "end") { if (depth > 0) depth--; next }
    if (line ~ /^[^ \t]+[ \t]+-->[ \t]+[^ \t]+$/) {
      split(line, e, /[ \t]+-->[ \t]+/); edge[e[1] " " e[2]] = 1; next
    }
    if (line ~ /^[A-Za-z0-9_.-]+(\[.*\])?$/) {
      id = line; sub(/\[.*$/, "", id)
      nodes[id]++
      where[id] = (depth > 0 ? stack[depth] : "")
    }
  }
  END {
    for (d in want) {
      if (!(d in parts)) { printf "30-domain-map: no file under ess/<n>-<layer>/ or ess/cloud/<n>-<layer>/ declares %s\n", d; bad++; continue }
      m = split(parts[d], ps, " ")
      for (i = 1; i <= m; i++) {
        p = ps[i]
        if (p in done) continue
        done[p] = 1
        if (nodes[p] != 1) { printf "30-domain-map: %s is %d node lines of the block, not 1\n", p, nodes[p] + 0; bad++; continue }
        if (where[p] != "L" layer[p]) { printf "30-domain-map: %s is in subgraph %s, not L%s\n", p, (where[p] == "" ? "(none)" : where[p]), layer[p]; bad++ }
      }
    }
    for (p in pair) {
      split(p, q, " ")
      if (p in edge) continue
      if ((q[1] in layer) && (q[2] in layer) && layer[q[1]] != layer[q[2]] && (("L" layer[q[1]] " L" layer[q[2]]) in edge)) continue
      printf "30-domain-map: reference %s -> %s is not drawn\n", q[1], q[2]; bad++
    }
    exit (bad > 0)
  }
' "$work/layers.txt" "$work/domains.txt" "$work/pairs.txt" "$work/committed.md" || bad=1

if [ "$bad" != 0 ]; then
  echo "30-domain-map: $page fails"
  exit 1
fi
echo "30-domain-map: $(wc -l < "$work/domains.txt" | tr -d ' ') domains and $(wc -l < "$work/pairs.txt" | tr -d ' ') references drawn in $page"
