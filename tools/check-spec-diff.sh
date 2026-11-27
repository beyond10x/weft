#!/bin/sh
# Classify the change of the specification since the last release and fail on a breaking or
# unknown change nobody acknowledged (technique 7 of the skill `ess:hardening`,
# docs/hardening/pass-3.md).
#
# Compares ess/ at the newest release tag (the highest `<major>.<minor>.<patch>` tag) with ess/ in
# the working tree: `ess verify diff --fail-on breaking-or-unknown`, all three dimensions (callers,
# readers, history). A change breaking or unknown in one of them fails with exit 4 unless
# quality/spec-diff-acknowledgements.json names its id. That file is bound to the digests of both
# revisions: a later change to ess/, or a new release tag, makes it stale, and a stale file fails
# with exit 1 until it is written again. A release removes it.
#
# Usage: tools/check-spec-diff.sh [--acknowledge | --negative]
#   (none)         the gate step: prints the classification and exits with the status of `ess`
#   --acknowledge  writes quality/spec-diff-acknowledgements.json naming every breaking or unknown
#                  change of this comparison (removes it when there is none); review the diff of
#                  that file before committing it
#   --negative     drops one actor grant in a copy of the release's ess/ and exits 0 only when the
#                  gate refuses that copy with exit 4
# SPEC_DIFF_BASE=<git ref> compares with that ref instead of the newest release tag.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$root/.check/spec-diff"
acks="quality/spec-diff-acknowledgements.json"
mode="${1:-}"
case "$mode" in
  "" | --acknowledge | --negative) ;;
  *) echo "usage: tools/check-spec-diff.sh [--acknowledge | --negative]" >&2; exit 2 ;;
esac

cd "$root"
base="${SPEC_DIFF_BASE:-$(git tag --list '[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname | head -n 1)}"
[ -n "$base" ] || { echo "check-spec-diff: no release tag (fetch the tags, or set SPEC_DIFF_BASE)" >&2; exit 2; }
rm -rf "$work"
mkdir -p "$work/base"
git archive "$base" ess | tar -x -C "$work/base" || {
  echo "check-spec-diff: cannot read ess/ at $base" >&2
  exit 2
}

if [ "$mode" = --negative ]; then
  cp -r "$work/base/ess" "$work/planted"
  grant='      - weft.infrastructure.CreateCloud'
  grep -qx "$grant" "$work/planted/9-actors/actors.yaml" || {
    echo "check-spec-diff --negative: the grant to drop is gone from actors.yaml; pick another" >&2
    exit 2
  }
  awk -v g="$grant" '$0 == g && !done { done = 1; next } { print }' "$work/base/ess/9-actors/actors.yaml" \
    > "$work/planted/9-actors/actors.yaml"
  status=0
  ess verify diff --from "$work/base/ess" --to "$work/planted" --fail-on breaking-or-unknown \
    > "$work/negative.txt" 2>&1 || status=$?
  if [ "$status" -eq 4 ] && grep -q 'grant-removed' "$work/negative.txt"; then
    echo "check-spec-diff --negative: a dropped grant of CreateCloud is refused (exit 4, grant-removed)"
    exit 0
  fi
  cat "$work/negative.txt" >&2
  echo "check-spec-diff --negative: a dropped grant passed the gate (exit $status)" >&2
  exit 1
fi

if [ "$mode" = --acknowledge ]; then
  ess verify diff --from "$work/base/ess" --to ess --compatibility --format json > "$work/diff.json"
  jq '{format: "ess-diff-acknowledgements/1", before: .before.spec_digest, after: .after.spec_digest,
       acknowledged: [.changes[] | select(.compatibility.verdict != "compatible") | .id]}' \
    "$work/diff.json" > "$work/acks.json"
  n="$(jq '.acknowledged | length' "$work/acks.json")"
  if [ "$n" -eq 0 ]; then
    rm -f "$acks"
    echo "check-spec-diff: nothing breaking or unknown since $base; $acks removed"
  else
    cp "$work/acks.json" "$acks"
    echo "check-spec-diff: $n breaking or unknown change(s) since $base acknowledged in $acks"
  fi
  exit 0
fi

set -- --from "$work/base/ess" --to ess --fail-on breaking-or-unknown
[ -f "$acks" ] && set -- "$@" --acknowledgements "$acks"
status=0
ess verify diff "$@" > "$work/gate.txt" 2>&1 || status=$?
grep -E '^([0-9]+ change|compatibility:|fails |gate |no semantic change)' "$work/gate.txt" || tail -n 20 "$work/gate.txt"
echo "check-spec-diff: ess/ against $base, exit $status"
case "$status" in
  0) ;;
  4) echo "check-spec-diff: a breaking or unknown change is not acknowledged; review it, then tools/check-spec-diff.sh --acknowledge" >&2 ;;
  *) echo "check-spec-diff: refused (a stale $acks is written again with --acknowledge); output in $work/gate.txt" >&2 ;;
esac
exit "$status"
