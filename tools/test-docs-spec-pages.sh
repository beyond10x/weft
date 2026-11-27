#!/bin/sh
# Usage: tools/test-docs-spec-pages.sh
# Self-test of tools/check.d/30-docs-spec-pages.sh: the hook exits 0 on the tree and 1 on each copy
# below, made under target/docs-spec-pages-test/:
#   component  ess/ with a fifth component (`weft-probe`, which takes `weft.metering`, its commands
#              and its events over from weft-control): the components table misses a row;
#   kind       ess/ with a twelfth variant of weft.catalog.OfferingKind (`ledger`): the
#              hand-written kinds table misses a row;
#   mode       the pages with one binding mode taken out of the generated block.
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
  /^  - name: weft\.catalog\.OfferingKind$/ { on = 1 }
  { print }
  on && /^      - queue$/ { print "      - ledger"; on = 0; done = 1 }
  END { if (!done) exit 1 }
' ess/7-services/catalog.yaml > "$work/kind/ess/7-services/catalog.yaml" || exit 2
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

[ "$failed" = 0 ] && echo "test-docs-spec-pages: every case holds"
exit "$failed"
