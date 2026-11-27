#!/bin/sh
# Check hook: the self-test of tools/check.d/20-rust-lint.sh (tools/test-rust-lint.sh): the hook
# on fixture repositories under target/test-rust-lint/, both sides of each check (editions,
# generated code included by a build script, every workspace, a missing Cargo.lock).
exec "$(dirname "$0")/../test-rust-lint.sh"
