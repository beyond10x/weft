#!/bin/sh
# Cases of tools/check.d/20-rust-lint.sh on fixture repositories (adversary round, story
# repo-lint-gate). Each case copies the hook unchanged into target/test-rust-lint/<case>/: a root
# workspace (members product/*/crates/*, edition 2021 in [workspace.package]), a tools/sim
# workspace of its own, each with a Cargo.lock, the pinned rust-toolchain.toml and a
# tools/synth.sh that does nothing; it runs the hook there and compares its exit status.
#
# Builds (cargo fmt, cargo clippy on crates without dependencies). Run by
# tools/check.d/21-rust-lint-selftest.sh inside the pool job of `slot task check`; alone:
# slot --timeout 10m tools/test-rust-lint.sh. Writes target/test-rust-lint/ only.
set -u
LC_ALL=C
export LC_ALL
repo=$(cd "$(dirname "$0")/.." && pwd)
work=$repo/target/test-rust-lint
fail=0
cases=0

# fixture <name>: a clean fixture repository at $work/<name>, path in $d.
fixture() {
  d=$work/$1
  rm -rf "$d"
  mkdir -p "$d/tools/check.d" "$d/product/p/crates/a/src" "$d/tools/sim/src" || exit 2
  cp -p "$repo/tools/check.d/20-rust-lint.sh" "$d/tools/check.d/" || exit 2
  printf '#!/bin/sh\nexit 0\n' > "$d/tools/synth.sh" && chmod +x "$d/tools/synth.sh" || exit 2
  cp "$repo/rust-toolchain.toml" "$d/" && cp "$repo/tools/sim/rust-toolchain.toml" "$d/tools/sim/" || exit 2
  printf '[workspace]\nresolver = "2"\nmembers = ["product/*/crates/*"]\nexclude = ["tools/sim"]\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\npublish = false\n' > "$d/Cargo.toml" || exit 2
  printf '[package]\nname = "a"\nversion.workspace = true\nedition.workspace = true\npublish.workspace = true\n' > "$d/product/p/crates/a/Cargo.toml" || exit 2
  printf '//! A.\n\n/// One.\npub fn one() -> u8 {\n    1\n}\n' > "$d/product/p/crates/a/src/lib.rs" || exit 2
  printf '[package]\nname = "sim"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n\n[workspace]\n' > "$d/tools/sim/Cargo.toml" || exit 2
  printf '//! Sim.\n\n/// One.\npub fn one() -> u8 {\n    1\n}\n' > "$d/tools/sim/src/lib.rs" || exit 2
  # A lock file cargo completes on its first run; the hook refuses a workspace without one.
  printf 'version = 4\n' > "$d/Cargo.lock" && printf 'version = 4\n' > "$d/tools/sim/Cargo.lock" || exit 2
}

# expect <want> <case> [<line>]: run the hook of fixture $d, compare its exit status with <want>;
# with <line>, its output must also hold that text (the reason it gave).
expect() {
  cases=$((cases + 1))
  "$d/tools/check.d/20-rust-lint.sh" > "$d.log" 2>&1 < /dev/null
  got=$?
  if [ "$got" != "$1" ]; then
    echo "FAIL: $2: want exit $1, got $got"
    grep '20-rust-lint:\|^error\|^warning' "$d.log" | sed 's/^/  | /' | head -n 12
    fail=1
  elif [ -n "${3:-}" ] && ! grep -qF "$3" "$d.log"; then
    echo "FAIL: $2: exit $got, but no line '$3'"
    grep '20-rust-lint:' "$d.log" | sed 's/^/  | /' | head -n 12
    fail=1
  else
    echo "ok: $2 (exit $got)"
  fi
}

# Control: the clean fixture passes.
fixture control
expect 0 "control: a clean fixture passes" "pass in: . tools/sim"

# A member crate whose own Cargo.toml names edition 2024 (what `cargo new` writes since Rust 1.85),
# in a workspace whose [workspace.package] names 2021. Its file formatted as cargo fmt formats it
# (style edition 2024 sorts `x9` before `x10`). Check 2 takes the edition of the crate, so it
# agrees with cargo fmt.
edition_2024() {
  mkdir -p "$d/product/p/crates/b/src" || exit 2
  printf '[package]\nname = "b"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n' > "$d/product/p/crates/b/Cargo.toml" || exit 2
  printf '//! B.\n\nmod m {\n    /// Nine.\n    pub fn x9() {}\n    /// Ten.\n    pub fn x10() {}\n}\n\npub use m::{%s};\n' "$1" > "$d/product/p/crates/b/src/lib.rs" || exit 2
}
fixture edition-2024
edition_2024 'x9, x10'
expect 0 "edition: a 2024 crate formatted by cargo fmt passes"
fixture edition-2024-as-2021
edition_2024 'x10, x9'
expect 1 "edition: a 2024 crate formatted as edition 2021 fails (cargo fmt --check)" "cargo fmt --check fails in ."

# Generated code reached through include! of a build script's output, the shape of weft-impl
# (build.rs writes OUT_DIR/weft_impl_generated.rs from target/generated, lib.rs includes it inside
# `#[allow(clippy::all)] mod generated`). The text is a function of the specification, not of
# anything written by hand; one enum of it with a variant 200 bytes larger than the rest
# (weft-impl's Row over the 121 entity snapshots) is a clippy warning.
# generated <large variant type> <lib.rs body after the header>
generated() {
  c=$d/product/p/crates/a
  printf '[package]\nname = "a"\nversion.workspace = true\nedition.workspace = true\npublish.workspace = true\nbuild = "build.rs"\n' > "$c/Cargo.toml" || exit 2
  printf '//! Writes the generated half of this crate.\n\nfn main() {\n    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));\n    std::fs::write(out.join("generated.rs"), GENERATED).expect("OUT_DIR is writable");\n}\n\nconst GENERATED: &str = "/// A row of any entity.\\n#[derive(Debug, Clone)]\\npub enum Row {\\n    /// A small row.\\n    Small(u8),\\n    /// A row of the entity with the most fields.\\n    Large(%s),\\n}\\n";\n' "$1" > "$c/build.rs" || exit 2
  printf '//! A.\n\n%s' "$2" > "$c/src/lib.rs" || exit 2
}
wrapped='/// Generated: not linted.
#[allow(clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub use generated::Row;
'
bare='include!(concat!(env!("OUT_DIR"), "/generated.rs"));
'
fixture generated-small
generated 'u16' "$wrapped"
expect 0 "generated: build-script output included into a member, rows of one size, passes"
fixture generated-large
generated '[u64; 64]' "$wrapped"
expect 0 "generated: the same output with one row 511 bytes larger is generated code, not linted"
# Why the module: the same output included at the crate root is linted.
fixture generated-large-bare
generated '[u64; 64]' "$bare"
expect 1 "generated: the large row included at the crate root, outside the module, fails (clippy)" "cargo clippy --workspace --all-targets -- -D warnings fails in ."
# The module allows clippy in the generated code only: a warning written by hand beside it fails.
fixture generated-beside-hand
generated '[u64; 64]' "$wrapped
/// Written by hand.
pub fn empty(v: &[u8]) -> bool {
    v.len() == 0
}
"
expect 1 "generated: a clippy warning written by hand beside the generated module fails" "cargo clippy --workspace --all-targets -- -D warnings fails in ."

# Every workspace of the repository is reached, not only . and tools/sim.
other() {
  mkdir -p "$d/tools/other/src" || exit 2
  printf '[package]\nname = "other"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n\n[workspace]\n' > "$d/tools/other/Cargo.toml" || exit 2
  printf '//! Other.\n\n/// Empty.\npub fn empty(v: &[u8]) -> bool {\n    %s\n}\n' "$1" > "$d/tools/other/src/lib.rs" || exit 2
}
fixture other-workspace
other 'v.is_empty()'
printf 'version = 4\n' > "$d/tools/other/Cargo.lock" || exit 2
expect 0 "workspaces: a third workspace, clean, is linted and passes" "pass in: . tools/other tools/sim"
fixture other-workspace-clippy
other 'v.len() == 0'
printf 'version = 4\n' > "$d/tools/other/Cargo.lock" || exit 2
expect 1 "workspaces: a clippy warning in a third workspace fails" "cargo clippy --workspace --all-targets -- -D warnings fails in tools/other"
fixture other-workspace-fmt
other 'v.is_empty( )'
printf 'version = 4\n' > "$d/tools/other/Cargo.lock" || exit 2
expect 1 "workspaces: a formatting change in a third workspace fails" "cargo fmt --check fails in tools/other"
fixture other-workspace-no-lock
other 'v.is_empty()'
expect 1 "workspaces: a workspace without a Cargo.lock fails (cargo would write one into the tree)" "tools/other has no Cargo.lock"
if [ -e "$d/tools/other/Cargo.lock" ]; then
  echo "FAIL: workspaces: the hook wrote tools/other/Cargo.lock"
  fail=1
fi

echo "test-rust-lint: $cases cases"
if [ "$fail" != 0 ]; then
  echo "test-rust-lint: fails"
  exit 1
fi
echo "test-rust-lint: pass"
