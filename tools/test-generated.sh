#!/bin/sh
# Self-test of the generated-code cache (tools/synth.sh) and its slow compare, run by `task check`
# as tools/check.d/05-generated-selftest.sh.
#
# Usage: tools/test-generated.sh
#
# On the fixture specification tools/sim/tests/fixture-from/ (synthesizes in about a second; the
# fixture tools/sim/tests/fixture/ is refused by ESS's Rust target, its domain `box` being a Rust
# keyword), with a cache under target/generated-selftest/:
#   cold        no cache: synthesized, both digest files hold, nothing else left behind;
#   warm        the cache is reused without calling ess (an ess that fails is given);
#   changed     a changed specification file or tools/synth-input.sh makes the cache stale: an ess
#               that fails is given and the run fails (the stale cache is not used), then the real
#               ess synthesizes and the new digest is recorded;
#   damaged     an edited, an added and a removed cache file are each found and synthesized again;
#   failed      an ess that writes part of its output and fails leaves the cache as it was, and no
#               temporary directory and no lock;
#   killed      a run killed (SIGKILL) during the synthesis leaves no cache; the next run removes
#               the lock and the temporary directory it left and synthesizes;
#   concurrent  two runs started together on no cache: one synthesizes, the other waits and reuses;
#   compare     tools/slow.d/10-generated-regenerate.sh passes on a clean cache and fails, naming
#               the file, on a cache whose digests hold but whose bytes another synthesis would
#               not write.
# Then, on ess/ itself, the transformation case: tools/synth-input.sh, and the difference between
# ess/ and target/synth-input/ read as parsed YAML (yq to JSON, compared with jq), which must be
# each transformation of tools/synth-input.sh at its count and nothing else. The counts are read
# from ess/.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
fixture=tools/sim/tests/fixture-from
work=target/generated-selftest
spec=$work/spec
cache=$work/cache
failed=0

pass() { echo "test-generated: ok   $1"; }
fail() { echo "test-generated: FAIL $1" >&2; failed=1; return 1; }
check() { if eval "$2"; then pass "$1"; else fail "$1"; fi; }

# run <log> <command...>: the command's output in <log>, its exit code in $rc.
run() {
  log=$1
  shift
  "$@" > "$log" 2>&1
  rc=$?
}
said() { grep -qF -- "$2" "$1"; }
show() { sed 's/^/  | /' "$1" >&2; }

# The cache holds: its SOURCES.sha256 is the digest of the sources, its files are OUTPUT.sha256's.
holds() {
  tools/synth.sh --digest --spec "$spec" | cmp -s - "$cache/SOURCES.sha256" &&
    (cd "$cache" && sha256sum --quiet --strict -c OUTPUT.sha256 >/dev/null 2>&1)
}
leftovers() { ls -d "$cache".tmp.* "$cache".old.* "$cache".lock 2>/dev/null; }

rm -rf "$work"
mkdir -p "$work"
cp -R "$fixture" "$spec"
cp -R "$fixture" "$work/spec.orig"
release=$(sed -n 's/^requires:[[:space:]]*ess[[:space:]]*\([^[:space:]]*\).*/\1/p' "$spec/ess-inputs.yaml")

# An ess that must not be called, and one that writes part of its output, then fails or hangs.
printf '#!/bin/sh\ntouch "%s/no-ess.called"\nexit 99\n' "$root/$work" > "$work/no-ess"
cat > "$work/part-ess" <<EOF
#!/bin/sh
[ "\$1" = --version ] && { echo "ess $release"; exit 0; }
while [ "\$#" -gt 0 ]; do [ "\$1" = --out ] && out=\$2; shift; done
mkdir -p "\$out/crates/part/src" && echo "// part" > "\$out/crates/part/src/lib.rs"
echo \$\$ > "$root/$work/part-ess.pid"
[ "\${PART_ESS:-fail}" = hang ] || exit 1
touch "$root/$work/part-ess.started"
exec sleep 30
EOF
chmod +x "$work/no-ess" "$work/part-ess"

# --- cold -------------------------------------------------------------------------------------
run "$work/cold.log" tools/synth.sh --spec "$spec" --out "$cache"
check "cold: no cache is synthesized (exit $rc)" '[ "$rc" = 0 ] && said "$work/cold.log" "no cache at $cache; synthesizing"' || show "$work/cold.log"
check "cold: SOURCES.sha256 is the digest of the sources, OUTPUT.sha256 the digest of the files" holds
check "cold: no .ess-output, no temporary directory, no lock left" '[ ! -e "$cache/.ess-output" ] && [ -z "$(leftovers)" ]'
cp -R "$cache" "$work/cache.good"

# --- warm -------------------------------------------------------------------------------------
run "$work/warm.log" env WEFT_SYNTH_ESS="$root/$work/no-ess" tools/synth.sh --spec "$spec" --out "$cache"
check "warm: the cache is reused without calling ess (exit $rc)" '[ "$rc" = 0 ] && said "$work/warm.log" "cache hit at $cache" && [ ! -e "$work/no-ess.called" ]' || show "$work/warm.log"

# --- changed ----------------------------------------------------------------------------------
echo "# changed by tools/test-generated.sh" >> "$spec/shelf.yaml"
run "$work/changed.log" env WEFT_SYNTH_ESS="$root/$work/no-ess" tools/synth.sh --spec "$spec" --out "$cache"
check "changed source: the stale cache is not used (exit $rc)" '[ "$rc" != 0 ] && said "$work/changed.log" "is stale" && [ -e "$work/no-ess.called" ]' || show "$work/changed.log"
check "changed source: the run that failed left the stale cache as it was" 'diff -r -q "$work/cache.good" "$cache" >/dev/null && [ -z "$(leftovers)" ]'
rm -f "$work/no-ess.called"
run "$work/changed2.log" tools/synth.sh --spec "$spec" --out "$cache"
check "changed source: synthesized again, the new digest recorded (exit $rc)" '[ "$rc" = 0 ] && said "$work/changed2.log" "is stale" && holds' || show "$work/changed2.log"
cp "$work/spec.orig/shelf.yaml" "$spec/shelf.yaml"
run "$work/changed3.log" tools/synth.sh --spec "$spec" --out "$cache"
check "changed source: back to the old source, back to the old cache (exit $rc)" '[ "$rc" = 0 ] && diff -r -q "$work/cache.good" "$cache" >/dev/null'

cp tools/synth-input.sh "$work/synth-input.sh"
echo "# changed by tools/test-generated.sh" >> "$work/synth-input.sh"
run "$work/input.log" env WEFT_SYNTH_INPUT="$work/synth-input.sh" WEFT_SYNTH_ESS="$root/$work/no-ess" tools/synth.sh --spec "$spec" --out "$cache"
check "changed tools/synth-input.sh: the stale cache is not used (exit $rc)" '[ "$rc" != 0 ] && said "$work/input.log" "is stale"' || show "$work/input.log"
rm -f "$work/no-ess.called"

echo "ess $release" > "$cache/SOURCES.sha256"
run "$work/recorded.log" env WEFT_SYNTH_ESS="$root/$work/no-ess" tools/synth.sh --spec "$spec" --out "$cache"
check "a cache whose recorded digest differs is not used (exit $rc)" '[ "$rc" != 0 ] && said "$work/recorded.log" "is stale"' || show "$work/recorded.log"
rm -f "$work/no-ess.called"
rm -rf "$cache"
cp -R "$work/cache.good" "$cache"

# --- damaged ----------------------------------------------------------------------------------
lib=$(cd "$cache" && find crates -name lib.rs | LC_ALL=C sort | head -n 1)
for how in edited added removed; do
  case $how in
    edited) echo "// edited by tools/test-generated.sh" >> "$cache/$lib" ;;
    added) echo "// added by tools/test-generated.sh" > "$cache/crates/added.rs" ;;
    removed) rm "$cache/$lib" ;;
  esac
  run "$work/damaged-$how.log" env WEFT_SYNTH_ESS="$root/$work/no-ess" tools/synth.sh --spec "$spec" --out "$cache"
  check "damaged ($how file): the cache is not used (exit $rc)" '[ "$rc" != 0 ] && said "$work/damaged-$how.log" "is damaged"' || show "$work/damaged-$how.log"
  rm -f "$work/no-ess.called"
  run "$work/damaged2-$how.log" tools/synth.sh --spec "$spec" --out "$cache"
  check "damaged ($how file): synthesized again, byte-equal to the clean cache (exit $rc)" '[ "$rc" = 0 ] && diff -r -q "$work/cache.good" "$cache" >/dev/null'
done

# --- failed -----------------------------------------------------------------------------------
echo "# changed by tools/test-generated.sh" >> "$spec/shelf.yaml"
run "$work/failed.log" env WEFT_SYNTH_ESS="$root/$work/part-ess" tools/synth.sh --spec "$spec" --out "$cache"
check "failed synthesis: the run fails (exit $rc)" '[ "$rc" != 0 ]'
check "failed synthesis: the cache is as it was, no temporary directory, no lock" 'diff -r -q "$work/cache.good" "$cache" >/dev/null && [ -z "$(leftovers)" ]'
cp "$work/spec.orig/shelf.yaml" "$spec/shelf.yaml"

# --- killed -----------------------------------------------------------------------------------
rm -rf "$cache" "$work/part-ess.started" "$work/part-ess.pid"
PART_ESS=hang WEFT_SYNTH_ESS="$root/$work/part-ess" tools/synth.sh --spec "$spec" --out "$cache" > "$work/killed.log" 2>&1 &
synth_pid=$!
n=0
while [ ! -e "$work/part-ess.started" ] && [ "$n" -lt 100 ]; do sleep 0.1; n=$((n + 1)); done
kill -9 "$synth_pid" 2>/dev/null
[ -f "$work/part-ess.pid" ] && kill -9 "$(cat "$work/part-ess.pid")" 2>/dev/null
wait "$synth_pid" 2>/dev/null
check "killed: the run was inside the synthesis when it was killed" '[ -e "$work/part-ess.started" ]'
check "killed: no cache, the partial output only in a temporary directory" '[ ! -e "$cache" ] && ls -d "$cache".tmp.* >/dev/null 2>&1 && [ -d "$cache.lock" ]'
run "$work/killed2.log" tools/synth.sh --spec "$spec" --out "$cache"
check "killed: the next run removes the lock and the leftovers and synthesizes (exit $rc)" '[ "$rc" = 0 ] && said "$work/killed2.log" "which is gone" && diff -r -q "$work/cache.good" "$cache" >/dev/null && [ -z "$(leftovers)" ]' || show "$work/killed2.log"

# --- concurrent -------------------------------------------------------------------------------
rm -rf "$cache"
tools/synth.sh --spec "$spec" --out "$cache" > "$work/concurrent-1.log" 2>&1 &
p1=$!
tools/synth.sh --spec "$spec" --out "$cache" > "$work/concurrent-2.log" 2>&1 &
p2=$!
wait "$p1"
r1=$?
wait "$p2"
r2=$?
synthesized=$(cat "$work/concurrent-1.log" "$work/concurrent-2.log" | grep -c "no cache at $cache; synthesizing")
reused=$(cat "$work/concurrent-1.log" "$work/concurrent-2.log" | grep -c "cache hit at $cache")
check "concurrent: both runs pass (exit $r1, $r2), one synthesizes, the other reuses its cache" '[ "$r1" = 0 ] && [ "$r2" = 0 ] && [ "$synthesized" = 1 ] && [ "$reused" = 1 ] && diff -r -q "$work/cache.good" "$cache" >/dev/null && [ -z "$(leftovers)" ]' ||
  { show "$work/concurrent-1.log"; show "$work/concurrent-2.log"; }

# --- compare ----------------------------------------------------------------------------------
run "$work/compare.log" tools/slow.d/10-generated-regenerate.sh "$spec" "$cache"
check "compare: a clean cache is byte-equal to a fresh synthesis (exit $rc)" '[ "$rc" = 0 ] && [ -z "$(ls -d "$cache".fresh.* 2>/dev/null)" ]' || show "$work/compare.log"
echo "// not what this ess writes" >> "$cache/$lib"
(cd "$cache" && payload=$(sed 's/^[0-9a-f]\{64\}  //' OUTPUT.sha256) && echo "$payload" | tr '\n' '\0' | xargs -0 sha256sum > OUTPUT.sha256)
run "$work/compare2.log" tools/slow.d/10-generated-regenerate.sh "$spec" "$cache"
check "compare: a cache whose digests hold but whose bytes differ fails, naming the file (exit $rc)" '[ "$rc" != 0 ] && said "$work/compare2.log" "cache hit at $cache" && said "$work/compare2.log" "$lib"' || show "$work/compare2.log"
rm -rf "$cache"

# --- the transformations of tools/synth-input.sh on ess/ --------------------------------------
yq=$(tools/synth-input.sh --yq) || { fail "a pinned yq"; exit 1; }
run "$work/input.log" tools/synth-input.sh ess target/synth-input
check "tools/synth-input.sh derives target/synth-input from ess/ (exit $rc)" '[ "$rc" = 0 ]' || show "$work/input.log"

(cd ess && find . -type f | LC_ALL=C sort) > "$work/files.ess"
(cd target/synth-input && find . -type f | LC_ALL=C sort) > "$work/files.input"
cmp -s "$work/files.ess" "$work/files.input" && pass "the input holds the files of ess/, no other" \
  || { fail "the input holds the files of ess/, no other"; diff "$work/files.ess" "$work/files.input" >&2; }

# One JSON document per file and side: {"file": <path>, "ess": <ess/ parsed>, "input": <target/synth-input/ parsed>}.
: > "$work/pairs.jsonl"
grep '\.yaml$' "$work/files.ess" | while IFS= read -r f; do
  f=${f#./}
  "$yq" -o=json -I=0 'explode(.)' "ess/$f" > "$work/a.json" || { echo "yq ess/$f" >&2; exit 1; }
  "$yq" -o=json -I=0 'explode(.)' "target/synth-input/$f" > "$work/b.json" || { echo "yq target/synth-input/$f" >&2; exit 1; }
  jq -c -n --arg file "$f" --slurpfile a "$work/a.json" --slurpfile b "$work/b.json" \
    '{file: $file, ess: $a[0], input: $b[0]}' >> "$work/pairs.jsonl"
done || fail "every YAML file of both sides parses"
for f in $(grep -v '\.yaml$' "$work/files.ess"); do
  cmp -s "ess/$f" "target/synth-input/$f" || fail "a file that is no YAML is copied unchanged: $f"
done

one_time_lines=$(grep -r --include='*.yaml' -c '^[[:space:]]*one_time_response:' ess | awk -F: '{ n += $NF } END { print n + 0 }')
jq -s -r --argjson lines "$one_time_lines" '
  ["r1-create-replica", "r1-submit-scheduling-request", "r4-bind-placement", "r4-assign",
   "r4-schedule-instance", "r4-record-placement-assigned", "edge-backend-ready",
   "edge-backend-restore"] as $removed
  | def otr: [.. | objects | select(has("one_time_response"))] | length;
    def drop_otr: walk(if type == "object" then del(.one_time_response) else . end);
    def expected($file):
      drop_otr
      | if $file == "components.yaml" and has("bindings") then
          .bindings |= map(select(.id as $id | $removed | index($id) | not)
                           | if (.on_failure | type) == "object" then .on_failure = "retry" else . end)
        else . end;
  (map(.ess | otr) | add) as $otr_ess
  | (map(.input | otr) | add) as $otr_input
  | (map(select(.file == "components.yaml"))[0]) as $c
  | ($c.ess.bindings // [] | map(.id)) as $ids_ess
  | ($c.input.bindings // [] | map(.id)) as $ids_input
  | ($ids_ess - $ids_input) as $gone
  | ($c.ess.bindings // [] | map(select(.id as $id | $removed | index($id) | not))) as $kept
  | ($kept | map(select((.on_failure | type) == "object")) | length) as $maps_ess
  | ($c.input.bindings // [] | map(select((.on_failure | type) == "object")) | length) as $maps_input
  | ($c.input.bindings // [] | map(select(.on_failure == "retry")) | length) as $retry_input
  | ($kept | map(select(.on_failure == "retry")) | length) as $retry_ess
  | [ (if $otr_ess == $lines and $otr_ess > 0
       then "ok   E1: \($otr_ess) one_time_response keys in ess/, one per `one_time_response:` line"
       else "FAIL E1: \($otr_ess) one_time_response keys in ess/, \($lines) `one_time_response:` lines" end),
      (if $otr_input == 0 then "ok   E1: none left in target/synth-input/"
       else "FAIL E1: \($otr_input) one_time_response keys left in target/synth-input/" end),
      (if ($gone | sort) == ($removed | sort) and ($ids_input | length) == ($ids_ess | length) - ($removed | length)
       then "ok   bindings: exactly the \($removed | length) removed, \($gone | join(", "))"
       else "FAIL bindings: removed \($gone), expected exactly \($removed)" end),
      (if $ids_input == ($kept | map(.id)) then "ok   bindings: the other \($ids_input | length) kept, in order"
       else "FAIL bindings: the kept bindings differ from ess/" end),
      (if $maps_ess > 0 and $maps_input == 0 and $retry_input == $retry_ess + $maps_ess
       then "ok   E2: all \($maps_ess) map-valued on_failure of the kept bindings set to retry, none left"
       else "FAIL E2: \($maps_ess) map-valued on_failure in ess/, \($maps_input) left, \($retry_input) retry (expected \($retry_ess + $maps_ess))" end),
      (map(.file as $f | select((.ess | expected($f)) != .input) | .file) as $changed
       | if $changed == [] then "ok   no other value changed in \(length) YAML files"
         else "FAIL no other value changed: \($changed | join(", ")) differ beyond the transformations" end)
    ]
  | .[]
' "$work/pairs.jsonl" > "$work/transformations.txt" || fail "the transformation case runs"
while IFS= read -r line; do
  case $line in
    ok*) pass "${line#ok   }" ;;
    *) fail "${line#FAIL }" ;;
  esac
done < "$work/transformations.txt"

if [ "$failed" != 0 ]; then
  echo "test-generated: failed" >&2
  exit 1
fi
echo "test-generated: all cases pass"
