#!/bin/sh
# Synthesize the conformance scenarios of one weft domain, fast enough for an edit loop.
#
# Builds .check/synth-<name>/ess/ with only the domain and the domains it needs, directly or
# through them: those whose types or relations its entities name (edges from
# tools/check-layers.sh), whose commands move its entities, whose rows its guards read and whose
# errors its commands answer with; plus `weft.shared`, `weft.actors` (grants of kept domains only)
# and the conversions of components.yaml. No components or bindings: this measures the domain's own
# scenarios. The gate synthesizes the whole system. The subset drops the vendored blocks'
# `one_time_response` declarations (below): a guarantee its scenarios do not check.
#
# Usage: tools/synth-domain.sh [--snapshot] [--validate] <name> [<path under ess/>...]
#   <name>      domain name without the prefix: cluster, access, ...
#   --snapshot  take files from .check/snapshot, the named paths from the working tree
#   --validate  build the subset and run `ess specify validate` on it only (no synthesis)
# Prints the synthesis summary, the refusal counts by code for this domain's outcomes, then runs the
# suite against the interpreted target and prints its summary and the failed scenarios of the domain
# (disclosure variants left out). SLOT_FLAGS defaults to `--timeout 10m`: a subset synthesizes in
# 80-130 s (2026-11-28, accounts 82 s, access 89 s, infrastructure 88 s, scheduling 125 s), so a run
# that reaches 10 minutes is a defect to report, not a job to wait for. SLOT_FLAGS=--local keeps
# the job on this host (a local `ess` build). Synthesis and the run go through `slot` with
# `--jobs 2` (2026-11-26 05:14 CET: a synthesis held 22 GB and the host ran out of memory). Run one
# at a time.
set -eu
# The subset lives under the git-ignored .check/. A job the local daemon hands to a build node
# syncs without git-ignored files unless this is set, and the node then reads the subset of an
# earlier run (seen 2026-11-28: a run step refused a suite that required an older ess).
export SLOT_SYNC_IGNORED=1

root="$(cd "$(dirname "$0")/.." && pwd)"
from="$root/ess"
validate=0
while :; do
  case "${1:-}" in
    --snapshot) from="$root/.check/snapshot" ;;
    --validate) validate=1 ;;
    *) break ;;
  esac
  shift
done
name="${1:?usage: tools/synth-domain.sh [--snapshot] [--validate] <name> [paths...]}"
shift
domain="weft.$name"

work="$root/.check/synth-$name"
full="$work/full"
tree="$work/ess"
rm -rf "$work"
mkdir -p "$full" "$tree"
cp -r "$from/." "$full/"
for p in "$@"; do
  mkdir -p "$full/$(dirname "$p")"
  cp "$root/ess/$p" "$full/$p"
done
# measure with whatever `ess` is on PATH: the copy requires that release
sed -i "s/^requires: ess .*/requires: ess $(ess --version | cut -d' ' -f2)/" "$full/ess-inputs.yaml"

# edges of the full tree
ess specify compile --path "$full" --format json > "$work/ir.json" 2> "$work/compile.err" || {
  cat "$work/compile.err" >&2
  exit 2
}
jq -r '
  (.entities | to_entries | map({key: .value.identity.type_ref.name, value: .value.domain})
    | map(select(.key != null)) | from_entries) as $owner
  | ((.types // {}) | to_entries
      | map({key: .key, value: [.value.body | .. | objects | select(.kind? == "declared") | .name]})
      | from_entries) as $direct
  | def closure:
      . as $s
      | ([$s[], ($s[] | ($direct[.] // [])[])] | unique) as $n
      | if ($n | length) == ($s | length) then $s else ($n | closure) end;
  .entities | to_entries[] | .value as $e
  | (($e.fields // [])[] | [.type_ref | .. | objects | select(.kind? == "declared") | .name] | unique | closure
       | .[] | select($owner[.] != null and $owner[.] != $e.domain) | "\($e.domain) \($owner[.])"),
    (($e.relations // [])[] | "\($e.domain) \(.target | split(".")[0:2] | join("."))")
' "$work/ir.json" > "$work/edges.raw"
# a domain whose command creates or moves a kept entity is kept too (ESS refuses a transition no
# kept command takes): edge from the entity's domain to the command's domain
jq -r '
  .commands | to_entries[] | .value as $c
  | $c.outcomes[]? | ((.affects // [])[].entity, (.subject.entity // empty))
  | split(".")[0:2] | join(".") | select(. != $c.domain) | "\(.) \($c.domain)"
' "$work/ir.json" >> "$work/edges.raw"
# a domain whose command reads a row of another domain in a guard (when_related, a row set) keeps
# that domain: weft's run-time files guard vendored commands on weft.cluster, weft.nodes and
# weft.pipelines rows
jq -r '
  .commands[] | .domain as $d
  | [(.outcomes // [])[] | .. | objects | .entity? | strings] | unique[]
  | split(".")[0:2] | join(".") | select(. != $d) | "\($d) \(.)"
' "$work/ir.json" >> "$work/edges.raw"
# and the domain of an error a command answers with (the vendored cluster commands answer with
# weft.topology errors, metering with weft.limits ones)
jq -r '
  .commands[] | .domain as $d | (.outcomes // [])[] | .error? | strings
  | split(".")[0:2] | join(".") | select(. != $d) | "\($d) \(.)"
' "$work/ir.json" >> "$work/edges.raw"
sort -u "$work/edges.raw" > "$work/edges.txt"

# transitive closure from the domain
echo "$domain" > "$work/keep.txt"
while :; do
  awk 'NR == FNR { keep[$1] = 1; next } ($1 in keep) { print $2 }' "$work/keep.txt" "$work/edges.txt" \
    | cat - "$work/keep.txt" | sort -u > "$work/keep.new"
  if cmp -s "$work/keep.new" "$work/keep.txt"; then break; fi
  mv "$work/keep.new" "$work/keep.txt"
done
grep -qx 'weft.shared' "$work/keep.txt" || echo 'weft.shared' >> "$work/keep.txt"
# actors carry the caller attributes guards read (hardening pass 1): keep them, with only the
# grants of kept domains (filtered below)
grep -qx 'weft.actors' "$work/keep.txt" || echo 'weft.actors' >> "$work/keep.txt"

# subset tree: system.yaml and ess-inputs.yaml list only the kept domains
sed -n '1,/^domains:$/p' "$full/system.yaml" > "$tree/system.yaml"
grep -E '^  - weft\.' "$full/system.yaml" | sed 's/^  - //' | while read -r d; do
  if grep -qx "$d" "$work/keep.txt"; then printf '  - %s\n' "$d" >> "$tree/system.yaml"; fi
done
sed -n '1,/^specification:$/p' "$full/ess-inputs.yaml" > "$tree/ess-inputs.yaml"
printf '  - system.yaml\n' >> "$tree/ess-inputs.yaml"
grep -E '^  - (cloud/)?[0-9]+-[a-z-]+/[a-z-]+\.yaml$' "$full/ess-inputs.yaml" | sed 's/^  - //' | while read -r p; do
  d="$(sed -n 's/^domain: *//p' "$full/$p" | head -n 1)"
  if grep -qx "$d" "$work/keep.txt"; then
    mkdir -p "$tree/$(dirname "$p")"
    if [ "$d" = 'weft.actors' ]; then
      awk -v keepf="$work/keep.txt" '
        BEGIN { while ((getline k < keepf) > 0) keep[k] = 1 }
        /^      - weft\.[a-z]+\./ { split($2, a, "."); if (!(("weft." a[2]) in keep)) next }
        { print }
      ' "$full/$p" > "$tree/$p"
    else
      cp "$full/$p" "$tree/$p"
    fi
    printf '  - %s\n' "$p" >> "$tree/ess-inputs.yaml"
  fi
done
printf 'scenarios: []\n' >> "$tree/ess-inputs.yaml"
# the conversions of components.yaml, without its components and bindings: weft.workloads and
# weft.identity store a StampedAt input as a Timestamp
sed -n '/^conversions:$/,/^components:$/p' "$full/components.yaml" | sed '$d' \
  > "$tree/components.yaml"
sed -i 's/^  - system.yaml$/  - system.yaml\n  - components.yaml/' "$tree/ess-inputs.yaml"
# The subset's copy of the vendored blocks drops their `one_time_response` declarations:
# weft.infrastructure ProvisionNode/provisioned, ProvisionPoolNode/provisioned and
# IssueJoinToken/issued (join_token), and weft.identity ExchangeToken/exchanged (token). ESS writes
# one disclosure scenario per secret, outcome and actor for each, most of them ending in failed
# witness searches. 2026-11-28, ess 0.99.2: the accounts subset ran past 1800 s with the three
# infrastructure ones and took 82 s without them (cloudspec: a subset of its own, over 120 s to
# 22 s). This removes a guarantee from the subset's scenarios: they do not check that those four
# responses disclose their secret once only. The whole-system synthesis keeps it; ess/cloud/ is
# never edited. weft's own declarations (weft.access) stay: the access subset takes 89 s with them.
for f in "$tree"/cloud/*/*.yaml; do
  [ -f "$f" ] || continue
  grep -n '^ *one_time_response: ' "$f" \
    | sed "s|^|synth-domain: dropped from ${f#"$tree"/}:|" >&2 || true
  sed -i '/^ *one_time_response: /d' "$f"
done

if [ "$validate" -eq 1 ]; then
  echo "domains: $(tr '\n' ' ' < "$work/keep.txt")"
  status=0
  ess specify validate --path "$tree" > "$work/validate.log" 2>&1 || status=$?
  tail -n 1 "$work/validate.log"
  echo "synth-domain: $domain, subset validate exit $status, log $work/validate.log"
  exit "$status"
fi

start=$(date +%s)
status=0
slot ${SLOT_FLAGS:---timeout 10m} ess verify conform synthesize --path "$tree" --jobs 2 --out "$work/suite.json" > "$work/synth.log" 2>&1 || status=$?
end=$(date +%s)

echo "domains: $(tr '\n' ' ' < "$work/keep.txt")"
tail -n 1 "$work/synth.log"
grep -E "^refused: refusal\[ESS-SYNTH-[0-9]+\]: outcome $domain\." "$work/synth.log" \
  | grep -oE 'ESS-SYNTH-[0-9]+' | sort | uniq -c || true
echo "synth-domain: $domain, synthesis exit $status, $((end - start)) s, log $work/synth.log"
[ "$status" -eq 0 ] || exit "$status"

# run the subset suite against the interpreted target (the model executes itself)
start=$(date +%s)
run=0
slot ${SLOT_FLAGS:---timeout 10m} ess verify conform run --target interpreted --path "$tree" --suite "$work/suite.json" --report-format 2 \
  --report-out "$work/report.json" > "$work/run.log" 2>&1 || run=$?
end=$(date +%s)
tail -n 1 "$work/run.log"
grep -E "^  (failed|error) [^ ]*$domain\." "$work/run.log" | grep -v "/disclosure/" || true
echo "synth-domain: $domain, run exit $run, $((end - start)) s, log $work/run.log"
exit "$run"
