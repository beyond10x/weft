#!/bin/sh
# Check hook: formatting and lints of every Rust workspace of the repository. Fails (exit 1)
# unless
#   1. `cargo fmt --check` exits 0 in each workspace;
#   2. `rustfmt --check` passes every .rs file of the repository, each with the edition of its
#      crate (the `edition` of the nearest Cargo.toml with a [package], or of its workspace's
#      [workspace.package] when the crate inherits it; 2015 when it names none). This reaches
#      the files cargo fmt does not: those a crate takes in through include! of a build script's
#      output (the hand-written src/behaviours/*.rs of weft-impl);
#   3. `cargo clippy --workspace --all-targets -- -D warnings` exits 0 in each workspace.
# Exit 2 when it cannot measure.
#
# The workspaces: the root of every Cargo.toml outside target/, node_modules/ and .git/, as
# `cargo locate-project --workspace` names it (today ., tools/budget-fixture and
# tests/fixtures/generated-probe), less the ones named in SKIP below, which still get check 2.
# Each must commit its Cargo.lock: cargo writes a missing one into the tree, and this hook
# writes only under target/. A workspace other than . builds into target/rust-lint/ws/<path>.
#
# Generated code is neither formatted nor linted. target/generated (tools/synth.sh) lies under
# target/, which no check reads, and is reached only as path dependencies: `cargo fmt` without
# --all formats workspace members only, and clippy lints through RUSTC_WORKSPACE_WRAPPER, which
# cargo applies to members only. What weft-impl's build.rs writes from it is included inside
# `#[allow(clippy::all)] mod generated` (product/cloud/crates/weft-impl/src/lib.rs); the
# behaviours its build.rs includes are hand-written and stay linted.
#
# The root workspace needs target/generated to resolve: tools/synth.sh makes it or reuses it by
# its digest (a cache hit after task test). rustfmt and clippy are added to the pinned toolchain
# when it lacks them (rust-toolchain.toml installs the minimal profile). Builds run inside the
# pool job of `slot task check`. Self-test: tools/test-rust-lint.sh (tools/check.d/21-*).
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/../.." && pwd -P)
cd "$root" || exit 2
work=target/rust-lint
mkdir -p "$work" || exit 2

# Workspaces checks 1 and 3 do not build, separated by blanks, each with its reason below; check
# 2 still formats their files.
SKIP='tests/fixtures/generated-probe'
# tests/fixtures/generated-probe: a model of the generated crates' form, built as a path
# dependency of weft-testkit (its Cargo.toml); it commits no Cargo.lock.

for c in rustfmt:rustfmt clippy:cargo-clippy; do
  if ! rustup which "${c#*:}" > /dev/null 2>&1; then
    rustup component add "${c%%:*}" || { echo "20-rust-lint: cannot add ${c%%:*} to the toolchain" >&2; exit 2; }
  fi
done

tools/synth.sh > /dev/null || { echo "20-rust-lint: tools/synth.sh failed" >&2; exit 2; }

# The value of `edition` in section <section> of manifest <file>: a year, `workspace` when it
# is inherited, nothing when absent.
edition_of() {
  awk -v want="$1" '
    /^[[:space:]]*\[/ { s = $0; gsub(/[[:space:]]/, "", s); next }
    s == want && /^[[:space:]]*edition[[:space:]]*(\.|=)/ {
      if ($0 ~ /workspace/) { print "workspace"; exit }
      v = $0; sub(/^[^"]*"/, "", v); sub(/".*$/, "", v); print v; exit
    }' "$2"
}

# Every manifest, its directory and the root of its workspace.
find . \( -name target -o -name node_modules -o -name .git \) -prune -o -type f -name Cargo.toml -print |
  sed 's|^\./||' | sort > "$work/manifests.txt" || exit 2
[ -s "$work/manifests.txt" ] || { echo "20-rust-lint: no Cargo.toml" >&2; exit 2; }
: > "$work/crates.txt"
: > "$work/roots.txt"
while read -r m; do
  d=$(dirname "$m")
  r=$(cd "$d" && cargo locate-project --workspace --message-format plain < /dev/null) ||
    { echo "20-rust-lint: cargo locate-project failed in $d" >&2; exit 2; }
  r=${r#"$root"/}
  r=$(dirname "$r")
  echo "$r" >> "$work/roots.txt"
  grep -q '^[[:space:]]*\[package\]' "$m" && echo "$d $r" >> "$work/crates.txt"
done < "$work/manifests.txt"
sort -u "$work/roots.txt" -o "$work/roots.txt" || exit 2

bad=0

# 2. every .rs file with the edition of its crate
: > "$work/by-edition.txt"
find . \( -name target -o -name node_modules -o -name .git \) -prune -o -type f -name '*.rs' -print |
  sed 's|^\./||' | sort > "$work/files.txt" || exit 2
if grep -q '[[:space:]]' "$work/files.txt"; then
  echo "20-rust-lint: a path of a .rs file holds a blank" >&2; exit 2
fi
while read -r f; do
  d=$(dirname "$f")
  crate=
  while :; do
    if awk -v d="$d" '$1 == d { f = 1 } END { exit !f }' "$work/crates.txt"; then crate=$d; break; fi
    [ "$d" = . ] && break
    d=$(dirname "$d")
  done
  if [ -z "$crate" ]; then
    echo "20-rust-lint: $f belongs to no crate"
    bad=1
    continue
  fi
  e=$(edition_of '[package]' "$crate/Cargo.toml")
  if [ "$e" = workspace ]; then
    ws=$(awk -v d="$crate" '$1 == d { print $2; exit }' "$work/crates.txt")
    e=$(edition_of '[workspace.package]' "$ws/Cargo.toml")
    [ -n "$e" ] || { echo "20-rust-lint: $crate inherits an edition $ws/Cargo.toml does not name" >&2; exit 2; }
  fi
  echo "${e:-2015} $f" >> "$work/by-edition.txt"
done < "$work/files.txt"
for e in $(cut -d' ' -f1 "$work/by-edition.txt" | sort -u); do
  # shellcheck disable=SC2046 # one argument per path: no path holds a blank
  rustfmt --edition "$e" --check -l $(awk -v e="$e" '$1 == e { print $2 }' "$work/by-edition.txt") > "$work/unformatted.txt"
  rc=$?
  if [ "$rc" = 1 ]; then
    sed "s|^$root/||; s|^|20-rust-lint: not formatted (edition $e): |" "$work/unformatted.txt"
    echo "20-rust-lint: rustfmt --check fails; run rustfmt --edition $e on those files"
    bad=1
  elif [ "$rc" != 0 ]; then
    echo "20-rust-lint: rustfmt --edition $e failed (exit $rc)" >&2; exit 2
  fi
done

# 1. and 3. in each workspace
linted=
while read -r ws; do
  case " $SKIP " in *" $ws "*) echo "20-rust-lint: $ws skipped (SKIP)"; continue ;; esac
  if [ ! -f "$ws/Cargo.lock" ]; then
    echo "20-rust-lint: $ws has no Cargo.lock; commit one, or name it in SKIP with the reason"
    bad=1
    continue
  fi
  if ! (cd "$ws" && cargo fmt --check < /dev/null); then
    echo "20-rust-lint: cargo fmt --check fails in $ws; run cargo fmt there"
    bad=1
  fi
  linted="$linted $ws"
  set --
  [ "$ws" = . ] || set -- --target-dir "$root/$work/ws/$(echo "$ws" | tr / -)"
  if ! (cd "$ws" && cargo clippy --workspace --all-targets --quiet "$@" -- -D warnings < /dev/null); then
    echo "20-rust-lint: cargo clippy --workspace --all-targets -- -D warnings fails in $ws"
    bad=1
  fi
done < "$work/roots.txt"

if [ "$bad" != 0 ]; then
  echo "20-rust-lint: fails"
  exit 1
fi
echo "20-rust-lint: $(wc -l < "$work/files.txt" | tr -d ' ') .rs files formatted; cargo fmt --check and cargo clippy -D warnings pass in:$linted"
