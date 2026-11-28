#!/bin/sh
# Usage: tools/test-domain-map.sh     (tools/check.d/31-domain-map-selftest.sh, in task check)
# Self-test of the domain map: tools/gen-domain-map.sh and its check hook
# tools/check.d/30-domain-map.sh, on copies of the page under target/ (DOMAIN_MAP_PAGE).
#   awk     the hook exits 0 on the committed tree under every awk the host has (mawk, the awk of
#           Debian and Ubuntu, gawk, busybox awk, original-awk, nawk; DOMAIN_MAP_AWKS names
#           others): the block must not depend on which awk made it.
#   order   the generator refuses a page whose closing marker comes before the opening one, and
#           leaves that page as it was.
#   fence   the hook fails when the block sits inside a code fence, where the site prints it as
#           text and draws no figure.
#   domain-missing, edge-within-layer, edge-between-layers
#           checks 2 and 3 of the hook, each alone: on a fixture tree under target/ whose
#           generator misses a domain ess has or drops one reference, and whose page that
#           generator wrote, the hook fails with that domain or reference, not with the byte
#           compare.
#   layers-upward, layers-cloud, layers-vendored-upward, layers-vendored-value
#           tools/check-layers.sh on fixture trees under target/, each the committed ess/ with one
#           probe file added: a weft file of layer 2 that refers to an entity of layer 8 is
#           refused; one that refers to an entity of ess/cloud/ is accepted (exit 0, the edge in
#           --graph); a vendored file under ess/cloud/ of the domain weft.accounts that refers to
#           weft's own weft.accounts.Account (layer 2) is refused, though both declare the same
#           domain, and so is one that names a value type of weft's metering.yaml.
# Runs `ess` (tools/check-layers.sh --graph, the hook) a few times; no network; about 15 s.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
page=website/docs/specification/layers.md
work=$root/target/domain-map-selftest
rm -rf "$work" && mkdir -p "$work" || exit 2
fail=0
cases=0
no() { echo "FAIL: $*"; fail=1; }
ok() { echo "ok: $*"; }
# a sed edit of the copy that must change it
edit() { # edit <file> <sed script>
  cp "$1" "$1.orig" && sed "$2" "$1.orig" > "$1" && ! cmp -s "$1.orig" "$1" && rm -f "$1.orig"
}

# tools/check-layers.sh on a copy of ess/ with one probe file: the layers of weft's own files and
# the one layer of the vendored files under ess/cloud/.
probe() { # probe <case> <path under ess/> <domain> <field type>: makes the tree $work/tree-<case>
  t=$work/tree-$1
  mkdir -p "$t/tools" "$t/ess/${2%/*}" && cp -R ess/. "$t/ess/" && cp tools/check-layers.sh "$t/tools/" || return 1
  printf '%s\n' \
    "domain: $3" \
    'types:' \
    "  - name: $3.ProbeId" \
    '    kind: newtype' \
    '    of: Uuid' \
    'entities:' \
    "  - name: $3.Probe" \
    '    naming:' \
    '      summary: A probe of tools/check-layers.sh.' \
    '    identity:' \
    '      name: probe_id' \
    "      type: $3.ProbeId" \
    '    fields:' \
    '      - name: target' \
    "        type: $4" \
    '    lifecycle:' \
    '      initial: Open' \
    '      states: [Open]' \
    '      terminal: [Open]' > "$t/ess/$2" || return 1
  edit "$t/ess/ess-inputs.yaml" "s#^scenarios:#  - $2\\
scenarios:#" || return 1
  grep -qx "  - $3" "$t/ess/system.yaml" || edit "$t/ess/system.yaml" "s#^domains:\$#domains:\\
  - $3#"
}
# layers <case> <exit> <text>: runs tools/check-layers.sh --graph in the tree; it must exit <exit>
# and print <text>
layers() {
  cases=$((cases + 1))
  (cd "$work/tree-$1" && tools/check-layers.sh --graph) > "$work/$1.out" 2>&1
  rc=$?
  if [ "$rc" = "$2" ] && grep -qF -- "$3" "$work/$1.out"; then
    ok "$1: tools/check-layers.sh exits $rc with: $3"
  else
    no "$1: tools/check-layers.sh exits $rc, expected $2 with \"$3\":"
    grep -v '^ *[0-9]* [a-z.]* -> ' "$work/$1.out" | sed -n '1,6p'
  fi
}
probe layers-upward 2-tenancy/probe.yaml weft.probe weft.shared.BudgetId || exit 2
layers layers-upward 1 'upward: weft.probe (layer 2) -> weft.metering (layer 8) via weft.probe.Probe.target'
probe layers-cloud 2-tenancy/probe.yaml weft.probe weft.shared.ScopeId || exit 2
layers layers-cloud 0 'weft.probe -> cloud.scope'
probe layers-vendored-upward cloud/3-scope/probe.yaml weft.accounts weft.shared.AccountId || exit 2
layers layers-vendored-upward 1 'upward: cloud.accounts (layer cloud) -> weft.accounts (layer 2) via weft.accounts.Probe.target'
probe layers-vendored-value cloud/3-scope/probe.yaml weft.accounts weft.metering.Currency || exit 2
layers layers-vendored-value 1 'upward: cloud.accounts (layer cloud) -> weft.metering (layer 8) via weft.accounts.Probe.target'

tools/check-layers.sh --graph > "$work/graph.txt" ||
  { echo "test-domain-map: tools/check-layers.sh --graph failed" >&2; exit 2; }

# awk: the hook on the committed tree, with `awk` bound to each implementation found
ran=0
for impl in ${DOMAIN_MAP_AWKS:-mawk gawk busybox original-awk nawk}; do
  p=$(command -v "$impl" 2> /dev/null) || continue
  bin=$work/awk-$impl
  mkdir -p "$bin" || exit 2
  if [ "$impl" = busybox ]; then
    "$p" awk 'BEGIN { exit 0 }' 2> /dev/null || continue
    printf '#!/bin/sh\nexec "%s" awk "$@"\n' "$p" > "$bin/awk" && chmod +x "$bin/awk" || exit 2
  else
    ln -s "$p" "$bin/awk" || exit 2
  fi
  ran=$((ran + 1))
  cases=$((cases + 1))
  if PATH="$bin:$PATH" tools/check.d/30-domain-map.sh > "$work/hook-$impl.out" 2>&1; then
    ok "awk: the hook passes under $impl ($p)"
  else
    no "awk: the hook fails on the committed tree under $impl ($p):"
    sed -n '1,6p' "$work/hook-$impl.out"
  fi
done
[ "$ran" -gt 0 ] || { echo "test-domain-map: no awk of ${DOMAIN_MAP_AWKS:-mawk gawk busybox original-awk nawk} on PATH" >&2; exit 2; }

# order: closing marker before the opening one
cases=$((cases + 1))
awk '
  /^<!-- domain-map -->$/ { print "<!-- /domain-map -->"; print; next }
  /^<!-- \/domain-map -->$/ { next }
  { print }
' "$page" > "$work/order.md" || exit 2
cp "$work/order.md" "$work/order.before" || exit 2
if DOMAIN_MAP_GRAPH="$work/graph.txt" DOMAIN_MAP_PAGE="$work/order.md" tools/gen-domain-map.sh > "$work/order.out" 2>&1; then
  no "order: tools/gen-domain-map.sh exits 0 on a page whose closing marker precedes the opening one"
fi
if cmp -s "$work/order.before" "$work/order.md"; then
  ok "order: the page is unchanged"
else
  no "order: tools/gen-domain-map.sh changed the page: $(wc -l < "$work/order.before" | tr -d ' ') lines before, $(wc -l < "$work/order.md" | tr -d ' ') after"
fi

# fence: the generated block inside a code fence
cases=$((cases + 1))
awk '
  /^<!-- domain-map -->$/ { print "````text" }
  { print }
  /^<!-- \/domain-map -->$/ { print "````" }
' "$page" > "$work/fence.md" || exit 2
if DOMAIN_MAP_PAGE="$work/fence.md" tools/check.d/30-domain-map.sh > "$work/fence.out" 2>&1; then
  no "fence: the hook exits 0 on a page whose block sits inside a code fence (no figure is drawn)"
else
  ok "fence: the hook fails"
fi

# Checks 2 and 3 of the hook, each alone: a fixture tree under $work/tree-<case> (a copy of ess/,
# the page and the three scripts) where the generator misses what ess has, and the page is
# regenerated by that generator, so the byte compare passes and only check 2 or 3 can fail.
fixture() { # fixture <case>: makes the tree $work/tree-<case>
  t=$work/tree-$1
  mkdir -p "$t/tools/check.d" "$t/${page%/*}" &&
    cp -R ess "$t/ess" &&
    cp tools/check-layers.sh tools/gen-domain-map.sh "$t/tools/" &&
    cp tools/check.d/30-domain-map.sh "$t/tools/check.d/" &&
    cp "$page" "$t/$page"
}
# expect <case> <message>: regenerates the page of the tree, runs its hook; the hook must exit 1
# with <message> and without the byte-compare failure
expect() {
  t=$work/tree-$1
  cases=$((cases + 1))
  if ! (cd "$t" && DOMAIN_MAP_GRAPH="$work/graph.txt" tools/gen-domain-map.sh) > "$work/$1.gen" 2>&1; then
    no "$1: the fixture's generator fails:"; sed -n '1,4p' "$work/$1.gen"; return
  fi
  (cd "$t" && tools/check.d/30-domain-map.sh) > "$work/$1.out" 2>&1
  rc=$?
  if [ "$rc" = 1 ] && grep -qF "$2" "$work/$1.out" && ! grep -q 'differs from' "$work/$1.out"; then
    ok "$1: the hook fails alone with: $2"
  else
    no "$1: the hook exits $rc, expected 1 with \"$2\" and no byte-compare failure:"
    sed -n '1,6p' "$work/$1.out"
  fi
}

# check 2: ess has a domain the generator does not draw. The generator takes the files of
# ess-inputs.yaml named `<n>-<layer>/<name>.yaml` with [a-z-] only; renamed to actors_v.yaml the
# domain still compiles and its file still declares it, but has no node.
fixture domain-missing || exit 2
edit "$work/tree-domain-missing/ess/ess-inputs.yaml" 's#^  - 9-actors/actors\.yaml$#  - 9-actors/actors_v.yaml#' ||
  { echo "test-domain-map: ess-inputs.yaml lists no 9-actors/actors.yaml" >&2; exit 2; }
mv "$work/tree-domain-missing/ess/9-actors/actors.yaml" "$work/tree-domain-missing/ess/9-actors/actors_v.yaml" || exit 2
expect domain-missing 'weft.actors is 0 node lines of the block, not 1'

# check 3: the generator drops one reference: a pair within one layer (drawn domain to domain),
# then the only pair between two layers (drawn as the layer edge). Both picked from the graph.
for f in ess/[0-9]*-*/*.yaml ess/cloud/[0-9]*-*/*.yaml; do
  d=$(sed -n 's/^domain:[ ]*//p' "$f" | head -n 1)
  [ -n "$d" ] || continue
  case $f in
    ess/cloud/*) printf '%s %s\n' "cloud.${d#*.}" cloud ;;
    *) dir=${f#ess/}; printf '%s %s\n' "$d" "${dir%%-*}" ;;
  esac
done > "$work/layers.txt"
awk 'NF == 0 { exit } NF == 4 && $3 == "->" { print $2, $4 }' "$work/graph.txt" | sort -u > "$work/pairs.txt"
same=$(awk 'NR == FNR { l[$1] = $2; next } l[$1] == l[$2] { print; exit }' "$work/layers.txt" "$work/pairs.txt")
sole=$(awk '
  NR == FNR { l[$1] = $2; next }
  l[$1] != l[$2] { k = l[$1] " " l[$2]; n[k]++; p[k] = $0 }
  END { for (k in n) if (n[k] == 1) print p[k] }
' "$work/layers.txt" "$work/pairs.txt" | sort | head -n 1)
for c in "edge-within-layer:$same" "edge-between-layers:$sole"; do
  name=${c%%:*}
  pair=${c#*:}
  if [ -z "$pair" ]; then
    no "$name: the graph has no such pair; pick another case"
    continue
  fi
  fixture "$name" || exit 2
  edit "$work/tree-$name/tools/gen-domain-map.sh" "s#| sort -u)\$#| sort -u | grep -vxF '$pair')#" ||
    { echo "test-domain-map: the pairs line of tools/gen-domain-map.sh ends in no | sort -u)" >&2; exit 2; }
  expect "$name" "reference ${pair% *} -> ${pair#* } is not drawn"
done

echo "test-domain-map: cases: $cases"
[ "$fail" = 0 ] || exit 1
