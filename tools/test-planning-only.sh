#!/bin/sh
# Both sides of tools/planning-only.sh, for the gate and for the docs build (--docs). The cases on
# this tree read the head side only (PLANNING_ONLY_BASE=-); the fixture repository below tests the
# base side.
here=$(dirname "$0")
fail=0
cases=0
expect() {
  want=$1; mode=$2; shift 2
  cases=$((cases + 1))
  if [ -n "$mode" ]; then
    printf '%s\n' "$@" | PLANNING_ONLY_BASE=- "$here/planning-only.sh" "$mode" >/dev/null
  else
    printf '%s\n' "$@" | PLANNING_ONLY_BASE=- "$here/planning-only.sh" >/dev/null
  fi
  got=$?
  if [ "$got" != "$want" ]; then echo "FAIL: want $want got $got for: ${mode:-gate} $*"; fail=1; fi
}
expect 0 "" .engineering/planning/design/components.md docs/hardening/pass-2.md README.md
expect 0 "" .engineering/project.yaml
expect 1 "" ess/5-runtime/nodes.yaml
expect 1 "" docs/hardening/pass-2.md tools/sim/src/main.rs
expect 1 "" .github/workflows/gate.yml
expect 1 "" Taskfile.yml
expect 1 "" ""
expect 0 "" website/docs/index.md website/docusaurus.config.ts README.md
expect 1 "" website/docs/index.md tools/planning-only.sh
# Adversary round (story docs-components-services): the generated blocks and the kinds table of
# these pages are checked only by tools/check.d/30-docs-spec-pages.sh, which runs in the gate; a
# pull request that edits only such a page must run it.
expect 1 "" website/docs/concepts/managed-services.md
expect 1 "" website/docs/concepts/components.md
# The marked pages are found by their markers, not from a list: the domain map of layers.md, and a
# page outside website/ that carries a marker. A page without one stays planning-only.
expect 1 "" website/docs/specification/layers.md
expect 0 "" website/docs/concepts/formations.md
marked=target/test-planning-only
mkdir -p "$here/../$marked" || exit 2
printf '# A\n\n<!-- generated: x -->\n<!-- /generated: x -->\n' > "$here/../$marked/generated.md" || exit 2
printf '# B\n\n<!-- domain-map -->\n<!-- /domain-map -->\n' > "$here/../$marked/domain-map.md" || exit 2
printf '# C\n\nText `<!-- generated: x -->` in a line.\n' > "$here/../$marked/plain.md" || exit 2
expect 1 "" "$marked/generated.md"
expect 1 "" "$marked/domain-map.md"
expect 0 "" "$marked/plain.md"
expect 0 --docs "$marked/generated.md"

# The base side: a pull request that takes the markers out of a page, or deletes a marked page,
# still runs the gate. A fixture repository under target/ holds a copy of planning-only.sh; its
# first commit (the base) has three marked pages, the head drops the markers of one, deletes
# another and keeps the third, plus one page without markers on both sides.
repo=$here/../$marked/repo
rm -rf "$repo" && mkdir -p "$repo/tools" "$repo/docs" || exit 2
cp "$here/planning-only.sh" "$repo/tools/" || exit 2
printf '# A\n\n<!-- generated: x -->\n<!-- /generated: x -->\n' > "$repo/docs/unmarked.md" &&
  printf '# B\n\n<!-- domain-map -->\n<!-- /domain-map -->\n' > "$repo/docs/deleted.md" &&
  printf '# C\n\n<!-- generated: y -->\n<!-- /generated: y -->\n' > "$repo/docs/kept.md" &&
  printf '# D\n\nNo block.\n' > "$repo/docs/plain.md" || exit 2
g() { git -C "$repo" -c user.name=test -c user.email=test@invalid -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@" > /dev/null 2>&1; }
g init -q && g add -A && g commit -q -m base || exit 2
printf '# A\n\nThe block is gone.\n' > "$repo/docs/unmarked.md" && rm "$repo/docs/deleted.md" || exit 2
g add -A && g commit -q -m head || exit 2
fixture() {
  want=$1; base=$2; shift 2
  cases=$((cases + 1))
  if [ "$base" = unset ]; then
    printf '%s\n' "$@" | (unset PLANNING_ONLY_BASE; "$repo/tools/planning-only.sh") > /dev/null
  else
    printf '%s\n' "$@" | PLANNING_ONLY_BASE=$base "$repo/tools/planning-only.sh" > /dev/null
  fi
  got=$?
  if [ "$got" != "$want" ]; then echo "FAIL: want $want got $got for: fixture base=$base $*"; fail=1; fi
}
fixture 1 HEAD~1 docs/unmarked.md
fixture 1 HEAD~1 docs/deleted.md
fixture 1 HEAD~1 docs/kept.md
fixture 0 HEAD~1 docs/plain.md
fixture 0 - docs/unmarked.md
# No PLANNING_ONLY_BASE and no origin/main to find the merge base with: the base cannot be read,
# so the change is not planning-only.
fixture 1 unset docs/plain.md
expect 0 --docs .engineering/project.yaml README.md docs/hardening/pass-2.md
expect 1 --docs website/docs/index.md
expect 1 --docs README.md website/package.json
expect 1 --docs .github/workflows/pages.yml
expect 1 --docs ""
[ "$fail" = 0 ] && echo "planning-only: $cases cases pass"
exit "$fail"
