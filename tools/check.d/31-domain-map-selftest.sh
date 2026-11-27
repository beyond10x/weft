#!/bin/sh
# Check hook: the self-test of the domain map (tools/test-domain-map.sh): the hook
# tools/check.d/30-domain-map.sh under every awk on PATH, and the marker cases on copies of the page.
exec "$(dirname "$0")/../test-domain-map.sh"
