#!/bin/sh
# Usage: tools/test-docs-check.sh     (task docs-check, the docs check step of pages.yml)
# Self-test of the hooks of tools/docs-check.d on the trees of tools/docs-check-fixture. For every
# <check>/pass and <check>/fail it runs the hook <check> (tools/docs-check.d/<check>.sh) with that
# tree as its argument and expects exit 0 on pass and exit 1 on fail, and a last line
# `<kpi-id> <count>` with count 0 on pass and above 0 on fail. When <check>/pass.out or
# <check>/fail.out exists, the whole output of the hook on that tree must equal it. Every
# executable hook has both trees and every fixture directory has its hook, so a new hook comes with
# its fixtures and edits neither this script nor tools/check-docs.sh. No network; ends in seconds.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/.." && pwd)
hooks=$root/tools/docs-check.d
fixtures=$root/tools/docs-check-fixture
fail=0
cases=0

no() { echo "FAIL: $*"; fail=1; }

hook_of() {
  if [ -x "$hooks/$1.sh" ]; then echo "$hooks/$1.sh"; elif [ -x "$hooks/$1" ]; then echo "$hooks/$1"; fi
}

[ -d "$fixtures" ] || { echo "test-docs-check: no directory $fixtures" >&2; exit 2; }
[ -d "$hooks" ] || { echo "test-docs-check: no directory $hooks" >&2; exit 2; }

for h in "$hooks"/*; do
  [ -f "$h" ] && [ -x "$h" ] || continue
  name=$(basename "$h" .sh)
  for side in pass fail; do
    [ -d "$fixtures/$name/$side" ] || no "hook $name has no fixture tree $name/$side"
  done
done

mkdir -p "$root/target" && out=$(mktemp "$root/target/docs-check.out.XXXXXX") || exit 2
trap 'rm -f "$out"' EXIT
for dir in "$fixtures"/*/; do
  name=$(basename "$dir")
  hook=$(hook_of "$name")
  if [ -z "$hook" ]; then no "fixture $name has no executable hook in tools/docs-check.d"; continue; fi
  for side in pass fail; do
    tree=$fixtures/$name/$side
    [ -d "$tree" ] || continue
    cases=$((cases + 1))
    "$hook" "$tree" > "$out" 2>&1
    got=$?
    want=0
    [ "$side" = fail ] && want=1
    [ "$got" = "$want" ] || { no "$name/$side: want exit $want got $got"; sed 's/^/  /' "$out"; continue; }
    last=$(tail -n 1 "$out")
    count=$(printf '%s\n' "$last" | sed -n 's/^[a-z][a-z0-9-]*\.[a-z0-9.-]* \([0-9][0-9]*\)$/\1/p')
    if [ -z "$count" ]; then
      no "$name/$side: last line is not '<kpi-id> <count>': $last"
    elif [ "$side" = pass ] && [ "$count" != 0 ]; then
      no "$name/$side: count $count, want 0"
    elif [ "$side" = fail ] && [ "$count" = 0 ]; then
      no "$name/$side: count 0, want above 0"
    fi
    if [ -f "$fixtures/$name/$side.out" ] && ! diff -u "$fixtures/$name/$side.out" "$out" > /dev/null; then
      no "$name/$side: output differs from $side.out"
      diff -u "$fixtures/$name/$side.out" "$out" | sed 's/^/  /'
    fi
  done
done

# More cases of one hook: every <check>/fail-<case> tree must make the hook <check> exit 1, every
# <check>/pass-<case> tree exit 0.
for tree in "$fixtures"/*/fail-*/ "$fixtures"/*/pass-*/; do
  [ -d "$tree" ] || continue
  tree=${tree%/}
  name=$(basename "$(dirname "$tree")")
  c=$(basename "$tree")
  hook=$(hook_of "$name")
  [ -n "$hook" ] || { no "$name/$c: no executable hook $name"; continue; }
  cases=$((cases + 1))
  want=0
  case "$c" in fail-*) want=1 ;; esac
  "$hook" "$tree" > "$out" 2>&1
  got=$?
  [ "$got" = "$want" ] || { no "$name/$c: want exit $want got $got"; sed 's/^/  /' "$out"; }
done

# A path that exists only as a file git ignores is not in the forge's clean checkout and not on
# github.com: the hook counts it on a local tree as the forge does (story acceptance 6, "in a clean
# checkout"; ~/beyond10x/AGENTS.md "The local gate equals the forge").
gitdir=$(mktemp -d "$root/target/docs-check.git.XXXXXX") || exit 2
trap 'rm -f "$out"; rm -rf "$gitdir"' EXIT
git_tree() {
  git -C "$1" init -q &&
    git -C "$1" add -A &&
    git -C "$1" -c user.name=fixture -c user.email=fixture@example.invalid \
      -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -qm fixture
}
ignored_case() { # ignored_case <hook> <tree>
  cases=$((cases + 1))
  git_tree "$2" > /dev/null 2>&1 || { no "$1/ignored-file: git failed"; return; }
  "$hooks/$1.sh" "$2" > "$out" 2>&1
  got=$?
  [ "$got" = 1 ] || { no "$1/ignored-file: want exit 1 got $got"; sed 's/^/  /' "$out"; }
}
t=$gitdir/10
mkdir -p "$t/website/docs" "$t/out"
printf '%s\n' 'out/' > "$t/.gitignore"
printf '%s\n' '# Fixture' '' 'https://github.com/beyond10x/weft/blob/main/out/report.txt' > "$t/website/docs/index.md"
printf '%s\n' 'report' > "$t/out/report.txt"
ignored_case 10-repo-links "$t"
t=$gitdir/30
mkdir -p "$t/build"
printf '%s\n' 'build/' > "$t/.gitignore"
printf '%s\n' '# Fixture' '' '- `build/report.txt`: the report.' > "$t/AGENTS.md"
printf '%s\n' 'report' > "$t/build/report.txt"
ignored_case 30-agents-md-paths "$t"
t=$gitdir/40
mkdir -p "$t"
cp -R "$fixtures/40-status-claims/pass/." "$t/"
mkdir -p "$t/out"
printf '%s\n' 'out/' > "$t/.gitignore"
printf '%s\n' 'report' > "$t/out/report.txt"
sed 's|"Later": \[|"Later": [\
      {"file": "out/report.txt"},|' "$fixtures/40-status-claims/pass/website/data/status-proofs.json" \
  > "$t/website/data/status-proofs.json"
ignored_case 40-status-claims "$t"

# An empty directory is not in a clean checkout either (git keeps no directory); a tracked file
# and a new file git would add (not ignored, not committed yet) are.
git_case() { # git_case <hook> <case> <want> <tree>
  cases=$((cases + 1))
  git_tree "$4" > /dev/null 2>&1 || { no "$1/$2: git failed"; return; }
  "$hooks/$1.sh" "$4" > "$out" 2>&1
  got=$?
  [ "$got" = "$3" ] || { no "$1/$2: want exit $3 got $got"; sed 's/^/  /' "$out"; }
}
t=$gitdir/30-empty
mkdir -p "$t/ess/scenarios"
printf '%s\n' '# Fixture' '' '- `ess/scenarios/`: none written.' > "$t/AGENTS.md"
git_case 30-agents-md-paths empty-directory 1 "$t"
t=$gitdir/30-new
mkdir -p "$t/docs"
printf '%s\n' '# Fixture' '' '- `docs/a.md` and `docs/new.md`.' > "$t/AGENTS.md"
printf '%s\n' 'a' > "$t/docs/a.md"
git_tree "$t" > /dev/null 2>&1
printf '%s\n' 'new' > "$t/docs/new.md"
cases=$((cases + 1))
"$hooks/30-agents-md-paths.sh" "$t" > "$out" 2>&1
got=$?
[ "$got" = 0 ] || { no "30-agents-md-paths/new-file: want exit 0 got $got"; sed 's/^/  /' "$out"; }

# 40-status-claims on the repository's own proofs and counts: website/, ess/ and Taskfile.yml are
# copied (a case changes them), the rest of the tree is linked.
real_tree() { # real_tree <dir>
  mkdir -p "$1/website"
  for e in "$root"/* "$root"/.github; do
    b=$(basename "$e")
    case "$b" in website | ess | Taskfile.yml | target) continue ;; esac
    ln -s "$e" "$1/$b"
  done
  cp -R "$root/website/data" "$root/website/docs" "$1/website/"
  cp "$root/website/product.json" "$1/website/"
  mkdir -p "$1/website/ux"
  cp -R "$root/website/ux/fixtures" "$1/website/ux/"
  cp -R "$root/ess" "$1/ess"
  cp "$root/Taskfile.yml" "$1/Taskfile.yml"
}
real_case() { # real_case <case> <want> <tree>
  cases=$((cases + 1))
  "$hooks/40-status-claims.sh" "$3" > "$out" 2>&1
  got=$?
  [ "$got" = "$2" ] || { no "40-status-claims/real-$1: want exit $2 got $got"; sed 's/^/  /' "$out"; }
}
t=$gitdir/40-real-baseline
real_tree "$t"
real_case baseline 0 "$t"
# "files" is no noun of the specification alone: cloudspec-sim's two output files are not 25.
t=$gitdir/40-real-other-files
real_tree "$t"
printf '%s\n' '# Results' '' 'cloudspec-sim writes 2 files: result.json and index.html.' > "$t/website/docs/zz-results.md"
real_case other-files 0 "$t"
# The gate no longer validates the specification: "The domain model" says it does.
t=$gitdir/40-real-spec-commented
real_tree "$t"
sed 's/^      - task: spec$/      # - task: spec/' "$root/Taskfile.yml" > "$t/Taskfile.yml"
real_case spec-commented 1 "$t"
# The spec task no longer runs ess: its command is a comment.
t=$gitdir/40-real-validate-commented
real_tree "$t"
sed 's/^      - ess specify validate --path ess --strict-requires$/      - echo validation off\
      # - ess specify validate --path ess --strict-requires/' "$root/Taskfile.yml" > "$t/Taskfile.yml"
real_case validate-commented 1 "$t"
# An empty directory is not in the forge's checkout and is no layer of ess/ess-inputs.yaml.
t=$gitdir/40-real-empty-layer-dir
real_tree "$t"
mkdir -p "$t/ess/9-old"
real_case empty-layer-dir 0 "$t"

# tools/check-docs.sh runs every hook, prints every KPI line and then fails with the highest exit
# (DOCS_CHECK_HOOKS names a directory of stub hooks).
stubs=$gitdir/hooks
mkdir -p "$stubs"
printf '#!/bin/sh\necho "stub.one 1"\nexit 1\n' > "$stubs/10-one.sh"
printf '#!/bin/sh\necho "stub: cannot measure" >&2\nexit 2\n' > "$stubs/20-two.sh"
printf '#!/bin/sh\necho "stub.three 0"\n' > "$stubs/30-three.sh"
chmod +x "$stubs"/*.sh
cases=$((cases + 1))
DOCS_CHECK_HOOKS=$stubs "$root/tools/check-docs.sh" > "$out" 2>&1
got=$?
if [ "$got" != 2 ] || ! grep -qx 'stub.one 1' "$out" || ! grep -qx 'stub.three 0' "$out"; then
  no "check-docs: want every hook run and exit 2, got exit $got"
  sed 's/^/  /' "$out"
fi

[ "$cases" -gt 0 ] || no "no case ran: no fixture tree with its hook under $fixtures"
[ "$fail" = 0 ] && echo "docs-check: $cases cases pass"
exit "$fail"
