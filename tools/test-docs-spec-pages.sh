#!/bin/sh
# Usage: tools/test-docs-spec-pages.sh
# Self-test of tools/check.d/30-docs-spec-pages.sh: the hook exits 0 on the tree and 1 on each copy
# below, made under target/docs-spec-pages-test/:
#   component  ess/ with a fifth component (`weft-probe`, which takes `weft.metering`, its commands
#              and its events over from weft-control): the components table misses a row;
#   kind       ess/ with a twelfth variant of weft.services.OfferingKind (`ledger`, in the copy of
#              the vendored file ess/cloud/8-services/services.yaml): the
#              hand-written kinds table misses a row;
#   mode       the pages with one binding mode taken out of the generated block;
#   kind-no-text, catalog-owner, services-owner, cells, placement   see each case below.
# Besides the hook, it checks the component cells against the IR and, with
# tools/docs-check.d/20-planned-banners.sh, that both pages keep their Planned banner.
# Each case must also print the reason it names, so a hook that fails for another reason (an
# `ess` that cannot compile the copy exits 2) does not pass. Runs `ess` (no network, about 1 s a
# case). Exit 1 when a case does not hold, 2 when it cannot set a case up.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
hook=tools/check.d/30-docs-spec-pages.sh
pages=website/docs/concepts
work=target/docs-spec-pages-test
rm -rf "$work" && mkdir -p "$work" || exit 2
failed=0

# expect <case> <exit> <text>, with DOCS_SPEC_ESS and DOCS_SPEC_PAGES set by the caller
expect() {
  DOCS_SPEC_SELFTEST=0 "$hook" > "$work/$1.out" 2>&1
  got=$?
  if [ "$got" != "$2" ]; then
    echo "test-docs-spec-pages: $1: exit $got, want $2"
    sed 's/^/  /' "$work/$1.out"
    failed=1
  elif [ -n "$3" ] && ! grep -qF -- "$3" "$work/$1.out"; then
    echo "test-docs-spec-pages: $1: exit $got but no line with \"$3\""
    sed 's/^/  /' "$work/$1.out"
    failed=1
  else
    echo "test-docs-spec-pages: ok: $1 (exit $got)"
  fi
}

DOCS_SPEC_ESS=ess DOCS_SPEC_PAGES=$pages expect tree 0 ''

mkdir -p "$work/component" && cp -R ess "$work/component/ess" || exit 2
awk '
  /^  - component: / { comp = $3 }
  /^    (owns|accepts|publishes):/ { sect = $1 }
  comp == "weft-control" && /^        - weft\.metering(\.|$)/ {
    if (sect == "accepts:") acc = acc $0 "\n"
    else if (sect == "publishes:") pub = pub $0 "\n"
    next
  }
  /^# Reconciliation loops/ && !done {
    printf "  - component: weft-probe\n    summary: A component the self-test adds.\n    owns:\n      domains:\n        - weft.metering\n"
    printf "    accepts:\n      commands:\n%s    publishes:\n      events:\n%s\n", acc, pub
    done = 1
  }
  { print }
  END { if (!done) exit 1 }
' ess/components.yaml > "$work/component/ess/components.yaml" || exit 2
DOCS_SPEC_ESS=$work/component/ess DOCS_SPEC_PAGES=$pages expect component 1 'components table: rows differ'

mkdir -p "$work/kind" && cp -R ess "$work/kind/ess" || exit 2
awk '
  /^  - name: weft\.services\.OfferingKind$/ { on = 1 }
  { print }
  on && /^      - queue$/ { print "      - ledger"; on = 0; done = 1 }
  END { if (!done) exit 1 }
' ess/cloud/8-services/services.yaml > "$work/kind/ess/cloud/8-services/services.yaml" || exit 2
DOCS_SPEC_ESS=$work/kind/ess DOCS_SPEC_PAGES=$pages expect kind 1 'kinds table: rows differ'

mkdir -p "$work/mode/pages" && cp "$pages/components.md" "$pages/managed-services.md" "$work/mode/pages/" || exit 2
awk '
  /^<!-- generated: binding-modes -->$/ { on = 1 }
  /^<!-- \/generated: binding-modes -->$/ { on = 0 }
  on && /^- `consume`/ && !done { done = 1; next }
  { print }
  END { if (!done) exit 1 }
' "$pages/managed-services.md" > "$work/mode/pages/managed-services.md" || exit 2
DOCS_SPEC_ESS=ess DOCS_SPEC_PAGES=$work/mode/pages expect mode 1 'generated block binding-modes differs'

# Cases written by an adversary round (story docs-components-services).

# kind-no-text: acceptance 4 wants one line of what each kind is for; a kinds row with its name
# and an empty second cell must fail the hook.
mkdir -p "$work/kind-no-text/pages" && cp "$pages/components.md" "$pages/managed-services.md" "$work/kind-no-text/pages/" || exit 2
awk '
  /^\| `queue` \|/ && !done { print "| `queue` | |"; done = 1; next }
  { print }
  END { if (!done) exit 1 }
' "$pages/managed-services.md" > "$work/kind-no-text/pages/managed-services.md" || exit 2
DOCS_SPEC_ESS=ess DOCS_SPEC_PAGES=$work/kind-no-text/pages expect kind-no-text 1 'kinds table'

# move_owner <case> <domain> <component>: a copy of ess/ in which <domain> moves from weft-control
# to the new <component>, and of both pages with the generator run on it, as an author would. The
# hand-written lines of managed-services.md and components.md still say weft-control owns it.
move_owner() {
  mkdir -p "$work/$1/pages" && cp -R ess "$work/$1/ess" &&
    cp "$pages/components.md" "$pages/managed-services.md" "$work/$1/pages/" || exit 2
  awk -v domain="$2" -v to="$3" '
    BEGIN { item = "        - " domain }
    /^  - component: / { comp = $3 }
    /^    (owns|accepts|publishes):/ { sect = $1 }
    comp == "weft-control" && ($0 == item || index($0, item ".") == 1) {
      if (sect == "accepts:") acc = acc $0 "\n"
      else if (sect == "publishes:") pub = pub $0 "\n"
      next
    }
    /^# Reconciliation loops/ && !done {
      printf "  - component: %s\n    summary: The domain %s, moved by the self-test.\n    owns:\n      domains:\n        - %s\n", to, domain, domain
      printf "    accepts:\n      commands:\n%s    publishes:\n      events:\n%s\n", acc, pub
      done = 1
    }
    { print }
    END { if (!done || acc == "") exit 1 }
  ' ess/components.yaml > "$work/$1/ess/components.yaml" || { echo "test-docs-spec-pages: $1: cannot move $2"; exit 2; }
  DOCS_SPEC_ESS=$work/$1/ess DOCS_SPEC_PAGES=$work/$1/pages tools/docs-spec-pages.sh > "$work/$1.gen" 2>&1 ||
    { echo "test-docs-spec-pages: $1: the generator failed"; sed 's/^/  /' "$work/$1.gen"; exit 2; }
}

# catalog-owner: weft.catalog moves to a new component; the pages still say weft-control owns the
# catalog, and the hook must not pass them.
move_owner catalog-owner weft.catalog weft-catalog
DOCS_SPEC_ESS=$work/catalog-owner/ess DOCS_SPEC_PAGES=$work/catalog-owner/pages expect catalog-owner 1 'for the catalog (weft.catalog)'

# services-owner: the same for the managed services, weft.services (vendored; its owner is named in
# weft's ess/components.yaml), which managed-services.md also describes.
move_owner services-owner weft.services weft-services
DOCS_SPEC_ESS=$work/services-owner/ess DOCS_SPEC_PAGES=$work/services-owner/pages expect services-owner 1 'for the managed services (weft.services)'

# placement-rule: components.md says "weft-control owns every domain with a command the node agent
# sends over its session or one of the control plane's own actors issues (the scheduler, the
# release controller); weft-api owns the user's other domains." Read with the actors it names,
# every domain weft-control owns must then have a command of NodeAgent (other than the node-local
# weft.node), Scheduler or ReleaseController. Checked while the page holds the sentence.
if grep -q 'owns every domain with a command' "$pages/components.md"; then
  ess specify compile --path ess --format json > "$work/placement-ir.json" 2> /dev/null || exit 2
  jq -r '. as $ir
    | ([$ir.actors["weft.actors.NodeAgent", "weft.actors.Scheduler", "weft.actors.ReleaseController"].may[]
        | split(".")[0:2] | join(".")] | unique - ["weft.node"]) as $rule
    | ($ir.components["weft-control"].owns - $rule)[]' "$work/placement-ir.json" > "$work/placement.out" || exit 2
  if [ -s "$work/placement.out" ]; then
    echo "test-docs-spec-pages: placement-rule: weft-control owns domains the rule of $pages/components.md gives to weft-api:"
    sed 's/^/  /' "$work/placement.out"
    failed=1
  else
    echo "test-docs-spec-pages: ok: placement-rule"
  fi
fi

# component-cells: acceptance 1, each row holds the owned domains and the number of accepted
# commands. The hook checks only the first cells without the generator; a generator that counts
# the wrong list (`.publishes`) regenerates a page the hook passes. Compared here with the IR,
# not through the generator.
ess specify compile --path ess --format json > "$work/cells-ir.json" 2> /dev/null || exit 2
jq -r '.components | keys[] as $k | .[$k] | "\($k) \(.owns | join(",")) \(.accepts | length)"' "$work/cells-ir.json" > "$work/cells.want" || exit 2
awk -F'|' '
  $0 == "<!-- generated: components -->" { on = 1; next }
  $0 == "<!-- /generated: components -->" { on = 0 }
  on && /^\| `/ { k = $2; d = $(NF - 2); n = $(NF - 1); gsub(/[` ]/, "", k); gsub(/[` ]/, "", d); gsub(/ /, "", n); print k, d, n }
' "$pages/components.md" > "$work/cells.got" || exit 2
if cmp -s "$work/cells.want" "$work/cells.got"; then
  echo "test-docs-spec-pages: ok: component-cells"
else
  echo "test-docs-spec-pages: component-cells: domains or command counts of $pages/components.md differ from the IR (< IR, > page):"
  diff "$work/cells.want" "$work/cells.got" | grep '^[<>]' | sed 's/^/  /'
  failed=1
fi

# banner: acceptance 9 and 11. With the Planned banner taken out of components.md, the docs check
# (docs.planned-without-banner) must count the page.
mkdir -p "$work/banner/website" && cp -R website/data website/docs "$work/banner/website/" || exit 2
awk '
  /^:::caution\[Planned\]/ { on = 1; done = 1; next }
  on && /^:::$/ { on = 0; next }
  !on { print }
  END { if (!done) exit 1 }
' "$pages/components.md" > "$work/banner/website/docs/concepts/components.md" || exit 2
tools/docs-check.d/20-planned-banners.sh "$work/banner" > "$work/banner.out" 2>&1
got=$?
if [ "$got" != 1 ]; then
  echo "test-docs-spec-pages: banner: 20-planned-banners exit $got on components.md without its banner, want 1"
  sed 's/^/  /' "$work/banner.out"
  failed=1
else
  echo "test-docs-spec-pages: ok: banner (exit $got)"
fi

# cells: a generator that counts the events a component publishes instead of the commands it
# accepts writes pages its own byte compare passes; the hook's check 3, which reads the IR and not
# the generator, must fail them.
mkdir -p "$work/cells/pages" && cp "$pages/components.md" "$pages/managed-services.md" "$work/cells/pages/" || exit 2
# The copy takes the working directory, the repository root, as its root: it lives under target/.
sed -e 's/(\.accepts | length)/(.publishes | length)/' -e 's|^root=.*|root=$(pwd)|' tools/docs-spec-pages.sh > "$work/cells/docs-spec-pages.sh" || exit 2
grep -q '(\.publishes | length)' "$work/cells/docs-spec-pages.sh" && grep -q '^root=\$(pwd)$' "$work/cells/docs-spec-pages.sh" ||
  { echo "test-docs-spec-pages: cells: cannot break the generator"; exit 2; }
chmod +x "$work/cells/docs-spec-pages.sh" || exit 2
DOCS_SPEC_PAGES=$work/cells/pages "$work/cells/docs-spec-pages.sh" > "$work/cells.gen" 2>&1
cmp -s "$pages/components.md" "$work/cells/pages/components.md" &&
  { echo "test-docs-spec-pages: cells: the broken generator changed nothing"; sed 's/^/  /' "$work/cells.gen"; exit 2; }
DOCS_SPEC_ESS=ess DOCS_SPEC_PAGES=$work/cells/pages DOCS_SPEC_GEN=$work/cells/docs-spec-pages.sh expect cells 1 'components table: cells differ'

# placement: the generated placement bullets say what the IR shows. In a copy of the IR,
# weft-control no longer accepts its first command and weft-node accepts the first command of
# weft-api; the generator's --check must then fail the tree's page with both changed bullets.
ess specify compile --path ess --format json > "$work/placement-base.json" 2> /dev/null || exit 2
jq '(.components["weft-control"].accepts) |= .[1:]
  | (.components["weft-node"].accepts) += [.components["weft-api"].accepts[0]]' "$work/placement-base.json" > "$work/placement-moved.json" || exit 2
DOCS_SPEC_IR=$work/placement-moved.json tools/docs-spec-pages.sh --check > "$work/placement-gen.out" 2>&1
got=$?
if [ "$got" = 1 ] &&
  grep -qF '> - Domains with commands that do not sit with exactly one component accepting all of them: `' "$work/placement-gen.out" &&
  grep -qF '> - Components that accept commands of a domain they do not own: `weft-node`.' "$work/placement-gen.out"; then
  echo "test-docs-spec-pages: ok: placement (exit $got)"
else
  echo "test-docs-spec-pages: placement: the generator's --check exit $got, want 1 with both changed bullets"
  sed 's/^/  /' "$work/placement-gen.out"
  failed=1
fi

[ "$failed" = 0 ] && echo "test-docs-spec-pages: every case holds"
exit "$failed"
