#!/bin/sh
# Both sides of tools/planning-only.sh, for the gate and for the docs build (--docs).
here=$(dirname "$0")
fail=0
cases=0
expect() {
  want=$1; mode=$2; shift 2
  cases=$((cases + 1))
  if [ -n "$mode" ]; then
    printf '%s\n' "$@" | "$here/planning-only.sh" "$mode" >/dev/null
  else
    printf '%s\n' "$@" | "$here/planning-only.sh" >/dev/null
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
expect 0 --docs .engineering/project.yaml README.md docs/hardening/pass-2.md
expect 1 --docs website/docs/index.md
expect 1 --docs README.md website/package.json
expect 1 --docs .github/workflows/pages.yml
expect 1 --docs ""
[ "$fail" = 0 ] && echo "planning-only: $cases cases pass"
exit "$fail"
