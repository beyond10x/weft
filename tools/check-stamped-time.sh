#!/bin/sh
# Fail when a command that takes a time stamped by weft-control is granted to a caller that could
# name its own time.
#
# A stamped time is an input whose type is weft.shared.StampedAt, directly or nested in a declared
# struct, union, newtype, list or optional type (ess/0-foundation/shared.yaml). Such a command may
# be granted only to the actors listed in tools/stamped-time-actors (weft-control's own actors, one
# name a line). The check also fails on a declared conversion into StampedAt (a plain Timestamp
# would then reach a stamped input through a binding) and on a listed actor the model does not
# declare. Read from `ess specify compile` output.
#
# Usage: tools/check-stamped-time.sh [--inventory | --negative]
#   --inventory  also print every Timestamp or StampedAt command input with the actors granted it.
#   --negative   run the check on a copy of ess/ whose NodeAgent is granted weft.nodes.Heartbeat
#                and exit 0 only when the check refuses that grant.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$root/.check/stamped-time"
trusted="$root/tools/stamped-time-actors"
mode="${1:-}"
mkdir -p "$work"

# check <spec dir> <work dir> <print inventory: 0|1>
check() {
  ess specify compile --path "$1" --format json > "$2/ir.json" 2> "$2/compile.err" || {
    cat "$2/compile.err" >&2
    return 2
  }
  jq -r --rawfile trusted "$trusted" '
    ($trusted | split("\n") | map(select(length > 0))) as $set
    | "weft.shared.StampedAt" as $stamp
    | ((.types // {}) | to_entries
        | map({key: .key, value: [.value.body | .. | objects | select(.kind? == "declared") | .name]})
        | from_entries) as $direct
    | def closure:
        . as $s
        | ([$s[], ($s[] | ($direct[.] // [])[])] | unique) as $n
        | if ($n | length) == ($s | length) then $s else ($n | closure) end;
      def stamped: [.. | objects | select(.kind? == "declared") | .name] | unique | closure | index($stamp) != null;
      def plain: [.. | objects | select(.kind? == "primitive" and .name? == "timestamp")] | length > 0;
    (.actors | to_entries | map(.key as $a | .value.may[]? | {c: ., a: $a})
      | group_by(.c) | map({key: .[0].c, value: map(.a)}) | from_entries) as $grants
    | (.actors | keys) as $declared
    | ($set[] | . as $a | select(($declared | index($a)) == null) | "BAD unknown actor in tools/stamped-time-actors: \(.)"),
      (.conversions[]? | select(.to | stamped) | "BAD conversion into \($stamp) from \(.from | tostring)"),
      (.commands[] | .name as $c | ($grants[$c] // []) as $g
        | (.input[]? | select(.type_ref | stamped or plain)
            | "IN \(if (.type_ref | stamped) then "stamped" else "plain" end) \($c).\(.name) [\($g | map(split(".")[-1]) | join(","))]"),
          (select([.input[]?.type_ref | stamped] | any) | $g[] | . as $a | select(($set | index($a)) == null)
            | "BAD granted outside the stamped-time set: \($c) to \(.)"))
  ' "$2/ir.json" > "$2/lines.txt" || { echo "check-stamped-time: jq failed on the compiled IR" >&2; return 2; }
  if [ "$3" = 1 ]; then
    sed -n 's/^IN //p' "$2/lines.txt"
  fi
  set_text="$(tr '\n' ' ' < "$trusted" | sed 's/ $//')"
  bad="$(grep -c '^BAD ' "$2/lines.txt" || true)"
  if [ "$bad" -gt 0 ]; then
    sed -n 's/^BAD //p' "$2/lines.txt"
    echo "$bad stamped-time violation(s); a command with a StampedAt input may be granted only to: $set_text"
    return 1
  fi
  inputs="$(grep -c '^IN stamped ' "$2/lines.txt" || true)"
  commands="$(sed -n 's/^IN stamped \([^ ]*\)\.[a-z_0-9]* .*/\1/p' "$2/lines.txt" | sort -u | wc -l | tr -d ' ')"
  echo "stamped-time: $inputs StampedAt input(s) in $commands command(s), granted only to $set_text"
}

case "$mode" in
  "") check "$root/ess" "$work" 0 ;;
  --inventory) check "$root/ess" "$work" 1 ;;
  --negative)
    neg="$work/negative"
    rm -rf "$neg"
    mkdir -p "$neg"
    cp -R "$root/ess" "$neg/ess"
    awk '
      /^  - name: weft\.actors\.NodeAgent$/ { inside = 1 }
      { print }
      inside && /^    may:$/ { print "      - weft.nodes.Heartbeat"; inside = 0 }
    ' "$root/ess/9-actors/actors.yaml" > "$neg/ess/9-actors/actors.yaml"
    grep -q '^      - weft.nodes.Heartbeat$' "$neg/ess/9-actors/actors.yaml" || {
      echo "check-stamped-time --negative: could not grant NodeAgent weft.nodes.Heartbeat in the copy" >&2
      exit 2
    }
    status=0
    check "$neg/ess" "$neg" 0 > "$neg/out.txt" 2>&1 || status=$?
    if [ "$status" = 1 ] && grep -q 'weft.nodes.Heartbeat to weft.actors.NodeAgent$' "$neg/out.txt"; then
      echo "stamped-time negative: a NodeAgent grant of weft.nodes.Heartbeat is refused"
    else
      cat "$neg/out.txt"
      echo "stamped-time negative: the check did not refuse a NodeAgent grant of weft.nodes.Heartbeat (exit $status)" >&2
      exit 1
    fi
    ;;
  *) echo "usage: tools/check-stamped-time.sh [--inventory | --negative]" >&2; exit 2 ;;
esac
