#!/bin/sh
# Setup hook: the generated code in target/generated/, synthesized from ess/ or reused when the
# digest of its sources holds (tools/synth.sh). task test runs this hook once more before it
# builds the test binaries, which need the generated crates as path dependencies.
exec "$(dirname "$0")/../synth.sh"
