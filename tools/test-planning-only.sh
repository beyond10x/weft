#!/bin/sh
# Both sides of tools/planning-only.sh.
here=$(dirname "$0")
fail=0
expect() {
  want=$1; shift
  printf '%s\n' "$@" | "$here/planning-only.sh" >/dev/null
  got=$?
  if [ "$got" != "$want" ]; then echo "FAIL: want $want got $got for: $*"; fail=1; fi
}
expect 0 .engineering/planning/design/components.md docs/hardening/pass-2.md README.md
expect 0 .engineering/project.yaml
expect 1 ess/5-runtime/nodes.yaml
expect 1 docs/hardening/pass-2.md tools/sim/src/main.rs
expect 1 .github/workflows/gate.yml
expect 1 Taskfile.yml
expect 1 ""
[ "$fail" = 0 ] && echo "planning-only: 7 cases pass"
exit "$fail"
