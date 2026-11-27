#!/bin/sh
# The metamorphic relations of hardening pass 3 (technique 5 of `ess:hardening`,
# docs/hardening/pass-3.md), each run by `weft-sim relate` from sim/experiments/full-setup.yaml
# settled by its controllers. Measures on the in-memory model, so it runs without slot; build
# weft-sim first (task sim-build).
#
# Usage: tools/sim-relations.sh [spec dir]   (default ess)
#   non-interference  globex's steps, a cross-account DefineWorkload into acme's formation among
#                     them, removed: every row naming an acme variable stays the same, except a
#                     revision `number` (pass 3 finding R1: minted from a counter of every request)
#   cordon-round-trip a node cordoned and uncordoned: every row stays the same
# Exit 1 when a relation does not hold.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
spec="${1:-ess}"
sim=tools/sim/target/debug/weft-sim
base=sim/experiments/full-setup.yaml
acme=tenant_a,account_a,sub_a,quota_a,sub_quota_a,cluster_a,topology_a,formation_shop,formation_etl,rev_shop,rev_etl,env_shop,env_etl
[ -x "$sim" ] || { echo "sim-relations: build weft-sim first (task sim-build)" >&2; exit 2; }
status=0
run() {
  name="$1"
  shift
  "$sim" relate --spec "$spec" --base "$base" --settle 12 "$@"
  rc=$?
  echo "sim-relations: $name exit $rc"
  [ "$rc" -eq 0 ] || status=1
}
run non-interference --base-append sim/relations/globex-defines-in-shop.yaml --drop globex,site --observe "$acme" --ignore number
run cordon-round-trip --derived-append sim/relations/cordon-uncordon.yaml
exit "$status"
