#!/bin/sh
# Check hook: formatting and lints of both Rust workspaces, the root workspace (members
# product/*/crates/*, sources under product/) and tools/sim (its own workspace). Fails (exit 1)
# unless, in each,
#   1. `cargo fmt --check` exits 0;
#   2. `rustfmt --check` passes every .rs file under its sources: this reaches the files cargo
#      fmt does not, those a crate takes in through include! of a build script's output (the
#      hand-written src/behaviours/*.rs of weft-impl);
#   3. `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
# Exit 2 when it cannot measure.
#
# Generated code is neither formatted nor linted: target/generated (tools/synth.sh) and
# tests/fixtures/generated-probe are excluded from the root workspace and reached only as path
# dependencies. `cargo fmt` without --all formats workspace members only, check 2 reads only the
# sources named above, and clippy runs its lints (and -D warnings) through
# RUSTC_WORKSPACE_WRAPPER, which cargo applies to members only. The root workspace needs
# target/generated to resolve: tools/synth.sh makes it or reuses it by its digest (a cache hit
# after task test). rustfmt and clippy are added to the pinned toolchain when it lacks them
# (rust-toolchain.toml installs the minimal profile).
#
# Builds run inside the pool job of `slot task check`; `cargo` takes its own slot through the
# shim on a workstation. Writes target/rust-lint/.
set -u
LC_ALL=C
export LC_ALL
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 2
work=target/rust-lint
mkdir -p "$work" || exit 2

for c in rustfmt:rustfmt clippy:cargo-clippy; do
  if ! rustup which "${c#*:}" > /dev/null 2>&1; then
    rustup component add "${c%%:*}" || { echo "20-rust-lint: cannot add ${c%%:*} to the toolchain" >&2; exit 2; }
  fi
done

tools/synth.sh > /dev/null || { echo "20-rust-lint: tools/synth.sh failed" >&2; exit 2; }

bad=0
# <workspace directory>:<its sources, relative to the repository root>
for pair in .:product tools/sim:tools/sim; do
  ws=${pair%%:*}
  src=${pair#*:}
  if ! (cd "$ws" && cargo fmt --check); then
    echo "20-rust-lint: cargo fmt --check fails in $ws; run cargo fmt there"
    bad=1
  fi

  edition=$(sed -n 's/^edition[[:space:]]*=[[:space:]]*"\([0-9]*\)".*/\1/p' "$ws/Cargo.toml" | head -n 1)
  [ -n "$edition" ] || { echo "20-rust-lint: no edition in $ws/Cargo.toml" >&2; exit 2; }
  find "$src" -name target -prune -o -type f -name '*.rs' -print | sort > "$work/files.txt" || exit 2
  [ -s "$work/files.txt" ] || { echo "20-rust-lint: no .rs file under $src" >&2; exit 2; }
  if grep -q '[[:space:]]' "$work/files.txt"; then
    echo "20-rust-lint: a path under $src holds a blank" >&2; exit 2
  fi
  # shellcheck disable=SC2046 # one argument per line of files.txt: no path holds a blank
  rustfmt --edition "$edition" --check -l $(cat "$work/files.txt") > "$work/unformatted.txt"
  rc=$?
  if [ "$rc" = 1 ]; then
    sed "s|^$root/||; s|^|20-rust-lint: not formatted: |" "$work/unformatted.txt"
    echo "20-rust-lint: rustfmt --check fails under $src; run rustfmt --edition $edition on those files"
    bad=1
  elif [ "$rc" != 0 ]; then
    echo "20-rust-lint: rustfmt failed under $src (exit $rc)" >&2; exit 2
  fi

  if ! (cd "$ws" && cargo clippy --workspace --all-targets --quiet -- -D warnings); then
    echo "20-rust-lint: cargo clippy --workspace --all-targets -- -D warnings fails in $ws"
    bad=1
  fi
done

if [ "$bad" != 0 ]; then
  echo "20-rust-lint: fails"
  exit 1
fi
echo "20-rust-lint: cargo fmt --check, rustfmt --check of every .rs file and cargo clippy -D warnings pass in . and tools/sim"
