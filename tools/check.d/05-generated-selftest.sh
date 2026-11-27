#!/bin/sh
# Check hook: the self-test of the generated-code cache (tools/test-generated.sh): the code ESS
# synthesizes is not committed but cached in target/generated, keyed by the digest of its sources
# (tools/synth.sh); the test step's setup hook tools/test-setup.d/10-generated.sh makes or reuses it.
exec "$(dirname "$0")/../test-generated.sh"
