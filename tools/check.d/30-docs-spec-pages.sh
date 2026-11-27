#!/bin/sh
# Check hook: the docs pages of the planned components and managed services
# (website/docs/concepts/components.md and managed-services.md) against the specification. Runs
# its self-test, tools/test-docs-spec-pages.sh, first (DOCS_SPEC_SELFTEST=0 skips it), then fails
# (exit 1) unless
#   1. every generated block, between `<!-- generated: <name> -->` and `<!-- /generated: <name> -->`,
#      is byte for byte the one tools/docs-spec-pages.sh makes from the specification now;
#   2. the first cells of the components table are the components of `ess specify compile`
#      (`.components | keys[]`), read here and not through the generator;
#   3. the first cells of the hand-written kinds table (the first table under `## Offering kinds`)
#      are the variants of weft.catalog.OfferingKind, in their order.
# DOCS_SPEC_ESS (default ess) names the specification, DOCS_SPEC_PAGES (default
# website/docs/concepts) the directory of the two pages. Writes under target/docs-spec-pages/.
# Exit 2 when it cannot measure. Runs `ess` (no network, about 1 s).
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
DOCS_SPEC_IR=$work/ir.json DOCS_SPEC_PAGES=$pages tools/docs-spec-pages.sh --check
case $? in
  0) ;;
  1) failed=1 ;;
  *) echo "30-docs-spec-pages: tools/docs-spec-pages.sh --check could not run" >&2; exit 2 ;;
esac

# first_cells <page> <awk condition that starts the table's region> <awk condition that ends it>:
# the first cell of each body row of the first table in the region, without its backticks.
first_cells() {
  awk -v start="$2" -v stop="$3" '
    !on && $0 == start { on = 1; next }
    on && stop != "" && index($0, stop) == 1 { exit }
    on && /^\|/ { rows++; if (rows > 2) { c = $0; sub(/^\|[ \t]*/, "", c); sub(/[ \t]*\|.*$/, "", c); gsub(/`/, "", c); print c } ; seen = 1; next }
    on && seen { exit }
  ' "$1"
}

# 2. the components table
jq -r '.components | keys[]' "$work/ir.json" > "$work/components.want" || exit 2
first_cells "$pages/components.md" '<!-- generated: components -->' '<!-- /generated: components -->' > "$work/components.got" || exit 2
if ! cmp -s "$work/components.want" "$work/components.got"; then
  echo "30-docs-spec-pages: $pages/components.md: components table: rows differ from .components of the specification (< specification, > page):"
  diff "$work/components.want" "$work/components.got" | grep '^[<>]'
  failed=1
fi

# 3. the hand-written kinds table
jq -r '.types["weft.catalog.OfferingKind"].body.variants[]' "$work/ir.json" > "$work/kinds.want" 2> /dev/null && [ -s "$work/kinds.want" ] ||
  { echo "30-docs-spec-pages: no weft.catalog.OfferingKind variants in $spec" >&2; exit 2; }
first_cells "$pages/managed-services.md" '## Offering kinds' '## ' > "$work/kinds.got" || exit 2
if ! cmp -s "$work/kinds.want" "$work/kinds.got"; then
  echo "30-docs-spec-pages: $pages/managed-services.md: kinds table: rows differ from weft.catalog.OfferingKind (< specification, > page):"
  diff "$work/kinds.want" "$work/kinds.got" | grep '^[<>]'
  failed=1
fi

[ "$failed" = 0 ] && echo "30-docs-spec-pages: both pages match $spec"
exit "$failed"
