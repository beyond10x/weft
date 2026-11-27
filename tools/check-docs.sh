#!/bin/sh
# Usage: tools/check-docs.sh     (task docs-check, the docs check step of pages.yml)
# Holds the docs claims KPIs at 0: runs every executable hook of tools/docs-check.d in name order
# (tools/run-hooks.sh) on the repository root. Each prints its offending items and
# `<kpi-id> <count>`, and fails when the count is above 0. Contract: tools/docs-check.d/README.md.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
exec tools/run-hooks.sh tools/docs-check.d
