#!/bin/sh
# Check hook: the vendored cloudspec blocks under ess/cloud/ match ess/cloud/cloud.lock and the pin
# CLOUDSPEC_REV, offline (tools/check-vendor-lock.sh); first its self-test on the fixture tree
# tools/fixtures/vendor-lock (tools/test-vendor-lock.sh), one case per refusal. No git, no network.
here=$(dirname "$0")
"$here/../test-vendor-lock.sh" || exit 1
exec "$here/../check-vendor-lock.sh"
