#!/bin/sh
# Fixture of tools/test-docs-check.sh: a tool that reads ess/inputs.yaml when it exists and writes
# out/generated.
[ -f ess/inputs.yaml ] || exit 0
mkdir -p out/generated
cp ess/inputs.yaml out/generated/
