#!/bin/sh
# Check hook: the docs pages of the planned components and managed services
# (website/docs/concepts/components.md and managed-services.md) against the specification. Runs
# its self-test, tools/test-docs-spec-pages.sh, first (DOCS_SPEC_SELFTEST=0 skips it), then fails
# (exit 1) unless
#   1. every generated block, between `<!-- generated: <name> -->` and `<!-- /generated: <name> -->`,
#      is byte for byte the one tools/docs-spec-pages.sh makes from the specification now;
#   2. the first cells of the components table are the components of `ess specify compile`
#      (`.components | keys[]`), read here and not through the generator;
#   3. each row of the components table holds, in its last two cells, the domains its component
#      owns and the number of commands it accepts, as the IR has them (read here, not through the
#      generator);
#   4. the first cells of the hand-written kinds table (the first table under `## Offering kinds`)
#      are the variants of weft.services.OfferingKind, in their order, and no row has an empty
#      second cell (what the kind is for);
#   5. the owners of the catalog and of the managed services are the ones the IR names: every
#      component that managed-services.md names in backticks outside its generated blocks owns
#      weft.services when its line holds "weft.services", and weft.catalog when its line holds
#      "weft.catalog" or does not hold "weft.services"; the page names the owner of each domain
#      at least once; every line of components.md outside its generated blocks that says
#      "catalog" names no component but the owner of weft.catalog.
# DOCS_SPEC_ESS (default ess) names the specification, DOCS_SPEC_PAGES (default
# website/docs/concepts) the directory of the two pages, DOCS_SPEC_GEN (default
# tools/docs-spec-pages.sh) the generator; the self-test sets it to a broken copy. Writes under
# target/docs-spec-pages.*/. Exit 2 when it cannot measure. Runs `ess` (no network, about 1 s).
# sh, awk (POSIX: mawk or gawk), jq and ess.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 2
if [ "${DOCS_SPEC_SELFTEST:-1}" != 0 ]; then
  tools/test-docs-spec-pages.sh || { echo "30-docs-spec-pages: tools/test-docs-spec-pages.sh failed"; exit 1; }
fi
spec=${DOCS_SPEC_ESS:-ess}
pages=${DOCS_SPEC_PAGES:-website/docs/concepts}
gen=${DOCS_SPEC_GEN:-tools/docs-spec-pages.sh}
mkdir -p target || exit 2
work=$(mktemp -d target/docs-spec-pages.XXXXXX) || exit 2
trap 'rm -rf "$work"' EXIT
for page in components.md managed-services.md; do
  [ -f "$pages/$page" ] || { echo "30-docs-spec-pages: no page $pages/$page" >&2; exit 2; }
done
ess specify compile --path "$spec" --format json > "$work/ir.json" 2> "$work/compile.err" ||
  { cat "$work/compile.err" >&2; exit 2; }
jq -e '.components | type == "object"' "$work/ir.json" > /dev/null 2>&1 ||
  { echo "30-docs-spec-pages: ess did not compile $spec" >&2; jq -r '.problems[]?' "$work/ir.json" >&2; exit 2; }

failed=0

# 1. the generated blocks
DOCS_SPEC_IR=$work/ir.json DOCS_SPEC_PAGES=$pages "$gen" --check
case $? in
  0) ;;
  1) failed=1 ;;
  *) echo "30-docs-spec-pages: $gen --check could not run" >&2; exit 2 ;;
esac

# rows <page> <line that starts the region> <line prefix that ends it>: the body rows of the first
# table in the region, one per line, each cell trimmed and without backticks, joined by a tab.
rows() {
  awk -v start="$2" -v stop="$3" '
    function trim(s) { gsub(/`/, "", s); sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
    !on && $0 == start { on = 1; next }
    on && stop != "" && index($0, stop) == 1 { exit }
    on && /^\|/ {
      rows++
      seen = 1
      if (rows <= 2) next
      line = $0
      gsub(/\\\|/, "\001", line)
      n = split(line, cell, "|")
      out = ""
      for (i = 2; i < n; i++) { c = trim(cell[i]); gsub(/\001/, "|", c); out = out (i > 2 ? "\t" : "") c }
      print out
      next
    }
    on && seen { exit }
  ' "$1"
}

# 2. and 3. the components table
rows "$pages/components.md" '<!-- generated: components -->' '<!-- /generated: components -->' > "$work/components.rows" || exit 2
jq -r '.components | keys[]' "$work/ir.json" > "$work/components.want" || exit 2
cut -f1 "$work/components.rows" > "$work/components.got" || exit 2
if ! cmp -s "$work/components.want" "$work/components.got"; then
  echo "30-docs-spec-pages: $pages/components.md: components table: rows differ from .components of the specification (< specification, > page):"
  diff "$work/components.want" "$work/components.got" | grep '^[<>]'
  failed=1
fi
jq -r '.components | keys[] as $k | .[$k] | "\($k)\t\(.owns | join(", "))\t\(.accepts | length)"' "$work/ir.json" > "$work/cells.want" || exit 2
awk -F '\t' '{ print $1 "\t" $(NF - 1) "\t" $NF }' "$work/components.rows" > "$work/cells.got" || exit 2
if ! cmp -s "$work/cells.want" "$work/cells.got"; then
  echo "30-docs-spec-pages: $pages/components.md: components table: cells differ from the owned domains and accepted commands of the specification (< specification, > page):"
  diff "$work/cells.want" "$work/cells.got" | grep '^[<>]'
  failed=1
fi

# 4. the hand-written kinds table
jq -r '.types["weft.services.OfferingKind"].body.variants[]' "$work/ir.json" > "$work/kinds.want" 2> /dev/null && [ -s "$work/kinds.want" ] ||
  { echo "30-docs-spec-pages: no weft.services.OfferingKind variants in $spec" >&2; exit 2; }
rows "$pages/managed-services.md" '## Offering kinds' '## ' > "$work/kinds.rows" || exit 2
cut -f1 "$work/kinds.rows" > "$work/kinds.got" || exit 2
if ! cmp -s "$work/kinds.want" "$work/kinds.got"; then
  echo "30-docs-spec-pages: $pages/managed-services.md: kinds table: rows differ from weft.services.OfferingKind (< specification, > page):"
  diff "$work/kinds.want" "$work/kinds.got" | grep '^[<>]'
  failed=1
fi
awk -F '\t' 'NF < 2 || $2 == "" { print $1 }' "$work/kinds.rows" > "$work/kinds.empty" || exit 2
if [ -s "$work/kinds.empty" ]; then
  echo "30-docs-spec-pages: $pages/managed-services.md: kinds table: no text of what the kind is for in the rows of:"
  sed 's/^/  /' "$work/kinds.empty"
  failed=1
fi

# 5. the owners of the catalog and of the managed services
# owner_of <domain>: its one owner in the IR.
owner_of() {
  jq -r --arg d "$1" '.components | to_entries[] | select(.value.owns | index($d)) | .key' "$work/ir.json" > "$work/owner" || exit 2
  [ "$(wc -l < "$work/owner")" -eq 1 ] || { echo "30-docs-spec-pages: $1 has no single owner in $spec" >&2; exit 2; }
  cat "$work/owner"
}
owner=$(owner_of weft.catalog) || exit 2
services_owner=$(owner_of weft.services) || exit 2
# named <page> <word>: each component in backticks on the lines outside the generated blocks
# (only on lines that hold <word>, when it is not empty), as `<line>\t<component>\t<domains>`,
# where <domains> names the domains the line speaks of: `services` on a line that holds
# "weft.services" and not "weft.catalog", `both` on a line that holds both, `catalog` otherwise.
named() {
  awk -v word="$2" -v list="$work/components.want" '
    BEGIN { while ((getline c < list) > 0) comp[c] = 1 }
    /^<!-- generated: / { gen = 1; next }
    /^<!-- \/generated: / { gen = 0; next }
    gen { next }
    word != "" && index($0, word) == 0 { next }
    {
      s = index($0, "weft.services") > 0
      c = index($0, "weft.catalog") > 0
      about = s && c ? "both" : s ? "services" : "catalog"
      rest = $0
      while (match(rest, /`[^`]+`/)) {
        t = substr(rest, RSTART + 1, RLENGTH - 2)
        if (t in comp) print NR "\t" t "\t" about
        rest = substr(rest, RSTART + RLENGTH)
      }
    }
  ' "$1"
}
named "$pages/managed-services.md" '' > "$work/owner.services" || exit 2
named "$pages/components.md" catalog > "$work/owner.components" || exit 2
awk -F '\t' -v owner="$owner" -v sowner="$services_owner" -v page="$pages/managed-services.md" '
  $3 != "services" && $2 != owner { print page ":" $1 ": names `" $2 "` for the catalog (weft.catalog)" }
  $3 != "catalog" && $2 != sowner { print page ":" $1 ": names `" $2 "` for the managed services (weft.services)" }
' "$work/owner.services" > "$work/owner.bad" || exit 2
awk -F '\t' -v owner="$owner" -v page="$pages/components.md" '$2 != owner { print page ":" $1 ": names `" $2 "` beside the catalog" }' "$work/owner.components" >> "$work/owner.bad" || exit 2
awk -F '\t' -v owner="$owner" '$3 != "services" && $2 == owner { n++ } END { exit !n }' "$work/owner.services" ||
  echo "$pages/managed-services.md: names no owner of the catalog (weft.catalog)" >> "$work/owner.bad"
awk -F '\t' -v sowner="$services_owner" '$3 != "catalog" && $2 == sowner { n++ } END { exit !n }' "$work/owner.services" ||
  echo "$pages/managed-services.md: names no owner of the managed services (weft.services)" >> "$work/owner.bad"
if [ -s "$work/owner.bad" ]; then
  echo "30-docs-spec-pages: the catalog (weft.catalog) is owned by \`$owner\` and the managed services (weft.services) by \`$services_owner\` in $spec; the pages say otherwise:"
  sed 's/^/  /' "$work/owner.bad"
  failed=1
fi

[ "$failed" = 0 ] && echo "30-docs-spec-pages: both pages match $spec"
exit "$failed"
