#!/bin/sh
# Derive the synthesis input of the Rust target from the unmodified specification.
#
# Usage: tools/synth-input.sh [<spec dir> [<out dir>]]     default: ess target/synth-input
#        tools/synth-input.sh --yq                          print the path of the pinned yq
#
# ESS 0.99.2 refuses the unmodified ess/ for `--target rust` (docs/implementation.md § Gaps E1-E3).
# This script copies <spec dir> to <out dir> (replaced) and applies exactly the transformations
# below, each with its ESS story (ESS store, commit bfb32ee3d6). <spec dir> is never written. When
# ESS ships a fix, its transformation here and the hand-written piece that restores it are removed
# together, in one change. tools/test-generated.sh reads the difference as parsed YAML and fails on
# anything else.
#
# E1, ESS story:the-rust-target-writes-a-one-time-response-outcome-as-an-obligation-that-carries-its-contract-instead-of-refusing-the-workspace
#   Every `one_time_response:` line of every YAML file is deleted.
# Bindings of the skeleton path removed; weft delivers them by hand with their declared policies
# (the generated System has no seam for a per-refusal policy):
#   r1-create-replica, ESS story:rust-binding-wraps-a-plain-copy-into-an-optional-member (E3)
#   r1-submit-scheduling-request, r4-bind-placement, r4-assign, r4-schedule-instance,
#   r4-record-placement-assigned, edge-backend-ready, edge-backend-restore, edge-backend-restarting,
#   ESS story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts (E2)
# E2 for every other binding, ESS story:the-rust-dispatch-selects-a-binding-failure-policy-per-refusal-of-the-invoked-command-and-counts-attempts
#   components.yaml is read with explode(.) (anchors and aliases resolved), and every map-valued
#   `on_failure` still present is set to `retry`.
#
# yq: v4.30.8. YQ names a binary to use as it is; else `yq` on PATH when it is v4.30.8; else the
# release binary, checked against its SHA-256, installed once under target/tools/.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
yq_version=v4.30.8
yq_sha256=6c911103e0dcc54e2ba07e767d2d62bcfc77452b39ebaee45b1c46f062f4fd26
removed_bindings='r1-create-replica r1-submit-scheduling-request r4-bind-placement r4-assign r4-schedule-instance r4-record-placement-assigned edge-backend-ready edge-backend-restore edge-backend-restarting'

pinned_yq() {
  if [ -n "${YQ:-}" ]; then echo "$YQ"; return; fi
  if command -v yq >/dev/null 2>&1 && yq --version 2>/dev/null | grep -q " version $yq_version\$"; then
    command -v yq
    return
  fi
  dir=$root/target/tools/yq-$yq_version
  if [ ! -x "$dir/yq" ]; then
    mkdir -p "$dir"
    curl -fsSL -o "$dir/yq.part" "https://github.com/mikefarah/yq/releases/download/$yq_version/yq_linux_amd64" </dev/null
    echo "$yq_sha256  $dir/yq.part" | sha256sum -c - >/dev/null || {
      rm -f "$dir/yq.part"
      echo "synth-input: the downloaded yq $yq_version does not match its SHA-256" >&2
      exit 1
    }
    chmod 0755 "$dir/yq.part"
    mv "$dir/yq.part" "$dir/yq"
  fi
  echo "$dir/yq"
}

if [ "${1:-}" = --yq ]; then
  pinned_yq
  exit 0
fi

spec=${1:-ess}
out=${2:-target/synth-input}
[ -d "$spec" ] || { echo "synth-input: no directory $spec" >&2; exit 2; }
yq=$(pinned_yq)

rm -rf "$out"
mkdir -p "$out"
cp -R "$spec/." "$out/"

# E1. Each key is one line in flow form; a block list would leave its items behind.
block=$(grep -rn --include='*.yaml' '^[[:space:]]*one_time_response:' "$out" | grep -v 'one_time_response:[[:space:]]*\[.*\][[:space:]]*\(#.*\)\{0,1\}$' || true)
[ -z "$block" ] || { echo "synth-input: a one_time_response that is not one line in flow form:" >&2; echo "$block" >&2; exit 1; }
grep -rl --include='*.yaml' '^[[:space:]]*one_time_response:' "$out" | LC_ALL=C sort | while IFS= read -r f; do
  sed '/^[[:space:]]*one_time_response:/d' "$f" > "$f.tmp"
  mv "$f.tmp" "$f"
done

# The bindings, and E2
components=$out/components.yaml
if [ -f "$components" ]; then
  select=
  for id in $removed_bindings; do
    select="${select:+$select or }.id == \"$id\""
  done
  "$yq" -i "explode(.) | del(.bindings[] | select($select)) | (.bindings[] | select(.on_failure | tag == \"!!map\") | .on_failure) = \"retry\"" "$components"
fi
