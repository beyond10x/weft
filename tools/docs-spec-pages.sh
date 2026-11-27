#!/bin/sh
# Usage: tools/docs-spec-pages.sh [--check]
# Writes the generated blocks of the docs pages of the planned components and managed services,
# website/docs/concepts/components.md and managed-services.md, from the specification. A block runs
# from the line `<!-- generated: <name> -->` to the line `<!-- /generated: <name> -->`; the rest of
# each page is written by hand. Blocks:
#   components.md        components      one row per component: its summary, the domains it owns
#                                        and the number of commands it accepts
#   managed-services.md  binding-modes   the variants of weft.catalog.BindingMode
#                        instance-states the lifecycle of weft.catalog.ServiceInstance
#                        plans           the summary and the fields of weft.catalog.OfferingPlan
#                        products        Product, ProductVersion and Installation, their summaries
# --check writes nothing: it prints each block that differs and exits 1.
# Reads the IR of `ess specify compile --path "$DOCS_SPEC_ESS" --format json` (DOCS_SPEC_ESS
# defaults to ess), or the compiled IR at DOCS_SPEC_IR; DOCS_SPEC_PAGES (default
# website/docs/concepts) names the directory of the pages. Writes its scratch files under target/.
# By hand: `slot --here tools/docs-spec-pages.sh`. Exit 2 when it cannot run. sh, awk, jq and ess.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
check=
case ${1:-} in
  --check) check=yes ;;
  '') ;;
  *) echo "usage: tools/docs-spec-pages.sh [--check]" >&2; exit 2 ;;
esac
pages=${DOCS_SPEC_PAGES:-website/docs/concepts}
mkdir -p target || exit 2
work=$(mktemp -d target/docs-spec-pages-gen.XXXXXX) || exit 2
trap 'rm -rf "$work"' EXIT
ir=${DOCS_SPEC_IR:-}
if [ -z "$ir" ]; then
  ir=$work/ir.json
  ess specify compile --path "${DOCS_SPEC_ESS:-ess}" --format json > "$ir" 2> "$work/compile.err" ||
    { cat "$work/compile.err" >&2; exit 2; }
fi
jq -e '.components | type == "object"' "$ir" > /dev/null 2>&1 || { echo "docs-spec-pages: no compiled specification in $ir" >&2; exit 2; }

# block <name>: the lines of one block, without its markers, from the IR.
block() {
  jq -r --arg name "$1" '
    def cell: tostring | gsub("\\|"; "\\|") | gsub("\n"; " ");
    def code: "`" + . + "`";
    def type_name: if .kind == "list" then "list of " + (.of | type_name)
      elif .kind == "optional" then "optional " + (.of | type_name)
      else (.name // .kind) end;
    if $name == "components" then
      "| Component | What it does | Domains it owns | Commands it accepts |",
      "|---|---|---|---|",
      (.components | keys[] as $k | .[$k] |
        "| \($k | code) | \(.naming.summary // "" | cell) | \(.owns | map(code) | join(", ")) | \(.accepts | length) |")
    elif $name == "binding-modes" then
      (.types["weft.catalog.BindingMode"].body.variants[] | "- \(code)")
    elif $name == "instance-states" then
      (.entities["weft.catalog.ServiceInstance"].lifecycle |
        "A service instance starts in \(.initial | code) and ends in \(.terminal | map(code) | join(", ")). Its states: \(.states | map(code) | join(", ")).",
        "",
        "| Transition | From | To |",
        "|---|---|---|",
        (.transitions[] | "| \(.name | code) | \(.from | map(code) | join(", ")) | \(.to | code) |"))
    elif $name == "plans" then
      (.entities["weft.catalog.OfferingPlan"] |
        "`OfferingPlan`: \(.naming.summary // "")",
        "",
        "A plan fixes:",
        "",
        (.fields[] | "- \(.name | code): \(.type_ref | type_name | code)"))
    elif $name == "products" then
      (.entities as $e | ("Product", "ProductVersion", "Installation") |
        "- \(code): \($e["weft.catalog." + .].naming.summary // "")")
    else error("no block " + $name) end
  ' "$ir"
}

# splice <page> <name> <block file> <out>: the page with the block between its markers replaced.
# Exit 1 when the page lacks the pair of markers, in order, each once.
splice() {
  awk -v name="$2" -v body="$3" '
    BEGIN { opener = "<!-- generated: " name " -->"; closer = "<!-- /generated: " name " -->" }
    $0 == opener { if (opens++ || closes) bad = 1; print; while ((getline line < body) > 0) print line; skip = 1; next }
    $0 == closer { if (!opens || closes++) bad = 1; skip = 0 }
    !skip { print }
    END { if (bad || opens != 1 || closes != 1) exit 1 }
  ' "$1" > "$4"
}

status=0
for spec in components.md:components managed-services.md:binding-modes managed-services.md:instance-states \
  managed-services.md:plans managed-services.md:products; do
  page=$pages/${spec%%:*}
  name=${spec#*:}
  [ -f "$page" ] || { echo "docs-spec-pages: no page $page" >&2; exit 2; }
  block "$name" > "$work/$name.block" || { echo "docs-spec-pages: cannot make block $name" >&2; exit 2; }
  splice "$page" "$name" "$work/$name.block" "$work/page.new" ||
    { echo "docs-spec-pages: $page: needs one line '<!-- generated: $name -->' and, after it, one line '<!-- /generated: $name -->'"; status=1; continue; }
  if cmp -s "$page" "$work/page.new"; then
    continue
  elif [ -n "$check" ]; then
    echo "docs-spec-pages: $page: generated block $name differs from the specification (< page, > regenerated):"
    diff "$page" "$work/page.new" | grep '^[<>]'
    status=1
  else
    cp "$work/page.new" "$page" || exit 2
    echo "docs-spec-pages: wrote block $name of $page"
  fi
done
exit "$status"
