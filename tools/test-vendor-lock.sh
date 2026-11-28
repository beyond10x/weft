#!/bin/sh
# Self-test of tools/check-vendor-lock.sh on copies of the fixture tree tools/fixtures/vendor-lock
# (a Taskfile.yml with CLOUDSPEC_REV, ess/ess-inputs.yaml, ess/cloud/ with its cloud.lock). The
# unchanged fixture passes; each case changes one thing in a fresh copy under
# target/test-vendor-lock and wants the check's exit code and a word of its message, so a case
# that fails for another reason than its own fails the test. No git, no network; under a second.
set -u
LC_ALL=C
export LC_ALL
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
fixture=$here/fixtures/vendor-lock
check=$here/check-vendor-lock.sh
work=$root/target/test-vendor-lock
rm -rf "$work" && mkdir -p "$work" || exit 2
fail=0
cases=0
n=0

# fresh: a new copy of the fixture, in a directory no case used before; sets t to its path. Runs
# in this shell (not in $(...)), so each case gets its own number.
fresh() {
  n=$((n + 1))
  t=$work/case-$n
  [ ! -e "$t" ] || { echo "test-vendor-lock: $t exists" >&2; exit 2; }
  cp -R "$fixture" "$t" || exit 2
}
# edit <file> <sed script>: change a file in place, portably.
edit() {
  sed "$2" "$1" > "$1.new" && mv "$1.new" "$1" || exit 2
}
# expect <want> <word> <name> <tree>: run the check on <tree>; its exit code is <want> and, when
# <word> is not empty, its standard error holds <word>.
expect() {
  want=$1; word=$2; name=$3; tree=$4
  cases=$((cases + 1))
  "$check" "$tree" > "$tree.out" 2> "$tree.err"
  got=$?
  if [ "$got" != "$want" ]; then
    echo "FAIL: $name: want exit $want, got $got"
    sed 's/^/  /' "$tree.err"
    fail=1
  elif [ -n "$word" ] && ! grep -qF -- "$word" "$tree.err"; then
    echo "FAIL: $name: exit $got, but the message does not say \"$word\":"
    sed 's/^/  /' "$tree.err"
    fail=1
  else
    echo "ok: $name (exit $got)"
  fi
}

fresh
expect 0 "" "the fixture as it is" "$t"

fresh; printf 'notes\n' > "$t/ess/cloud/README.md" || exit 2
expect 0 "" "a file under ess/cloud that is not YAML" "$t"

# Acceptance 1 of story:cloud-blocks-lock-gate, one case each.
fresh; edit "$t/ess/cloud/cloud.lock" 's#^format .*#format cloudspec-vendor/1#'
expect 1 "format" "a format other than cloudspec-vendor/2" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" 's/^system weft$/system cloud/'
expect 1 "system" "a system other than weft" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" 's/^commit .*/commit 4444444444444444444444444444444444444444/'
expect 1 "CLOUDSPEC_REV" "a lock commit that differs from CLOUDSPEC_REV" "$t"

fresh; edit "$t/ess/cloud/0-protocols/shared.yaml" '1s/weft/weFt/'
expect 1 "0-protocols/shared.yaml differs from its digest" "one changed byte in a listed file" "$t"

fresh; mkdir -p "$t/ess/cloud/5-workloads" && printf 'domain: weft.claims\n' > "$t/ess/cloud/5-workloads/claims.yaml" || exit 2
expect 1 "not in the lock: 5-workloads/claims.yaml" "an unlisted YAML file" "$t"

fresh; edit "$t/ess/ess-inputs.yaml" '\#^  - cloud/2-infrastructure/topology.yaml$#d'
expect 1 "ess-inputs.yaml" "an output missing from ess-inputs.yaml" "$t"

# The other side of each: the same refusal reached another way.
fresh; edit "$t/ess/cloud/cloud.lock" '/^format /d'
expect 1 "format" "no format line" "$t"

fresh; printf 'format cloudspec-vendor/2\n' >> "$t/ess/cloud/cloud.lock" || exit 2
expect 1 "format" "two format lines" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" '/^system /d'
expect 1 "system" "no system line" "$t"

fresh; edit "$t/Taskfile.yml" 's/^  CLOUDSPEC_REV: .*/  CLOUDSPEC_REV: 4444444444444444444444444444444444444444/'
expect 1 "CLOUDSPEC_REV" "a CLOUDSPEC_REV that differs from the lock commit" "$t"

fresh; edit "$t/Taskfile.yml" '/CLOUDSPEC_REV/d'
expect 1 "CLOUDSPEC_REV" "a Taskfile.yml without CLOUDSPEC_REV" "$t"

fresh; edit "$t/Taskfile.yml" 's/^  CLOUDSPEC_REV: /  # CLOUDSPEC_REV: /'
expect 1 "CLOUDSPEC_REV" "a CLOUDSPEC_REV that is commented out" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" 's/^commit .*/worktree 1111111111111111111111111111111111111111/'
expect 1 "commit" "a lock made from a working tree, not a commit" "$t"

fresh; rm "$t/ess/cloud/2-infrastructure/topology.yaml" || exit 2
expect 1 "2-infrastructure/topology.yaml is listed in the lock and missing" "a listed file that is missing" "$t"

fresh; printf '\n' >> "$t/ess/cloud/2-infrastructure/topology.yaml" || exit 2
expect 1 "2-infrastructure/topology.yaml differs from its digest" "one byte appended to a listed file" "$t"

fresh; printf 'domain: weft.claims\n' > "$t/ess/cloud/claims.yml" || exit 2
expect 1 "not in the lock: claims.yml" "an unlisted .yml file" "$t"

fresh; edit "$t/ess/ess-inputs.yaml" 's#^  - cloud/2-infrastructure/topology.yaml$#  \# - cloud/2-infrastructure/topology.yaml#'
expect 1 "ess-inputs.yaml" "an output listed in ess-inputs.yaml only in a comment" "$t"

fresh; edit "$t/ess/ess-inputs.yaml" '\#^  - cloud/2-infrastructure/topology.yaml$#d'
edit "$t/ess/ess-inputs.yaml" '/^scenarios: \[\]$/d'
printf 'scenarios:\n  - cloud/2-infrastructure/topology.yaml\n' >> "$t/ess/ess-inputs.yaml" || exit 2
expect 1 "ess-inputs.yaml" "an output listed in ess-inputs.yaml only under scenarios" "$t"

fresh; rm "$t/ess/cloud/cloud.lock" || exit 2
expect 1 "cloud.lock" "no lock" "$t"

fresh; printf 'checksum abc\n' >> "$t/ess/cloud/cloud.lock" || exit 2
expect 1 "line" "a line of a kind the format does not have" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" 's#^output \([0-9a-f]*\) 0-protocols/shared.yaml$#output \1 ../ess-inputs.yaml#'
expect 1 "line" "an output path outside ess/cloud" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" 's#^output 6602986efe81f80c47dd1a1c7b7730f4518b4819f98aa9511f32b5d12cd08896 #output 6602986efe81f80c47dd1a1c7b7730f4518b4819f98aa9511f32b5d12cd0889 #'
expect 1 "line" "an output digest that is not 64 hex digits" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" '\#^source .* blocks/2-infrastructure/topology.yaml$#d'
expect 1 "source" "an output without its source line" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" 's#^exclude components.yaml$#exclude 0-protocols/shared.yaml#'
expect 1 "excludes 0-protocols/shared.yaml and lists it as an output" "an exclude that the lock also lists as an output" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" '/^exclude /d'
expect 1 "exclude" "a lock without exclude lines" "$t"

fresh; edit "$t/ess/cloud/cloud.lock" '/^exclude components.yaml$/d'
expect 1 "exclude" "a lock that leaves out only one of the excludes of D-a" "$t"

fresh; printf 'exclude 7-edge/edge.yaml\n' >> "$t/ess/cloud/cloud.lock" || exit 2
expect 1 "exclude" "a lock with an exclude beyond D-a" "$t"

# --excludes prints the excludes of D-a, one per line; task vendor-update passes them to vendor.sh.
cases=$((cases + 1))
got=$("$check" --excludes | tr '\n' ' ')
if [ "$got" = "13-actors/actors.yaml components.yaml " ]; then
  echo "ok: --excludes prints the excludes of D-a"
else
  echo "FAIL: --excludes printed: $got"
  fail=1
fi

[ "$fail" = 0 ] && echo "vendor-lock: $cases cases pass"
exit "$fail"
