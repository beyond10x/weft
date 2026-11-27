#!/bin/sh
# Check hook: the self-test of the generated code's checks (tools/test-generated.sh), before the
# digest check of the committed tree (10-generated-digest.sh).
exec "$(dirname "$0")/../test-generated.sh"
