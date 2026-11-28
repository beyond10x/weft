#!/bin/sh
# Synthesize the conformance scenarios of one weft domain, fast enough for an edit loop.
#
# Builds .check/synth-<name>/ess/ with only the domain and the domains it references, directly or
# through them (edges from tools/check-layers.sh), plus `weft.shared` and `weft.actors` (grants of kept domains only). No components or bindings:
# this measures the domain's own scenarios. The gate synthesizes the whole system.
#
# Usage: tools/synth-domain.sh [--snapshot] <name> [<path under ess/>...]
#   <name>      domain name without the prefix: cluster, access, ...
#   --snapshot  take files from .check/snapshot, the named paths from the working tree
# Prints the synthesis summary, the refusal counts by code for this domain's outcomes, then runs the
# suite against the interpreted target and prints its summary and the failed scenarios of the domain
# (disclosure variants left out). SLOT_FLAGS defaults to `--timeout 30m` (2026-11-26: the build node ends a job at 10 min unless it names its time); SLOT_FLAGS=--local keeps the job on this host (a local `ess` build). Synthesis and the run go through `slot` with `--jobs 2` (2026-11-26 05:14 CET: a synthesis held 22 GB and the host ran out of memory). Run one at a time.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
from="$root/ess"
if [ "${1:-}" = "--snapshot" ]; then
  from="$root/.check/snapshot"
  shift
fi
name="${1:?usage: tools/synth-domain.sh [--snapshot] <name> [paths...]}"
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
# the vendored cloudspec blocks (ess/cloud/, below every weft layer) go into the subset whole,
# before the compile step, and their domains are always kept
if [ -d "$full/cloud" ]; then
  cp -r "$full/cloud" "$tree/cloud"
  sed -n 's/^domain: *//p' "$full"/cloud/*/*.yaml | sort -u > "$work/vendored.txt"
else
  : > "$work/vendored.txt"
fi

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

cat "$work/vendored.txt" "$work/keep.txt" | sort -u > "$work/keep.new" && mv "$work/keep.new" "$work/keep.txt"

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

start=$(date +%s)
status=0
slot ${SLOT_FLAGS:---timeout 30m} ess verify conform synthesize --path "$tree" --jobs 2 --out "$work/suite.json" > "$work/synth.log" 2>&1 || status=$?
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
slot ${SLOT_FLAGS:---timeout 30m} ess verify conform run --target interpreted --path "$tree" --suite "$work/suite.json" --report-format 2 \
  --report-out "$work/report.json" > "$work/run.log" 2>&1 || run=$?
end=$(date +%s)
tail -n 1 "$work/run.log"
grep -E "^  (failed|error) [^ ]*$domain\." "$work/run.log" | grep -v "/disclosure/" || true
echo "synth-domain: $domain, run exit $run, $((end - start)) s, log $work/run.log"
exit "$run"
