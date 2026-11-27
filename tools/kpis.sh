#!/bin/sh
# Usage: tools/kpis.sh     (task kpis)
# Prints the KPIs of design:quality-kpis that are measured on this machine: every executable file
# of tools/kpis.d, in name order (tools/run-hooks.sh), prints one line per KPI, its id, its value
# with the unit and the command that measured it, or `unavailable` and the reason. It measures, so
# it starts without the slot prefix; a hook puts the prefix on the commands inside it that build
# or test. Never part of task check. Hook contract: tools/kpis.d/README.md.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root" || exit 2
mkdir -p target/kpis
exec tools/run-hooks.sh tools/kpis.d
