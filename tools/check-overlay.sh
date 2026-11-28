#!/bin/sh
# Validate the whole weft system as it was frozen in .check/snapshot/, with only the named files
# taken from the working tree. Lets several sessions edit different domain files at once: each
# validates its own files against a stable copy of the others.
# The snapshot holds the vendored files under ess/cloud/ with the rest of ess/, so every overlay
# validates against them. They are cloudspec's blocks, changed only by `task vendor-update`: a
# path under cloud/ is refused (exit 2); after a vendor update, freeze again.
# Usage: tools/check-overlay.sh <tag> <path under ess/>...   e.g. check-overlay.sh me 2-tenancy/accounts.yaml
# Freeze a snapshot first: tools/check-overlay.sh --freeze
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
snap="$root/.check/snapshot"

if [ "${1:-}" = "--freeze" ]; then
  rm -rf "$snap"
  mkdir -p "$snap"
  cp -r "$root/ess/." "$snap/"
  echo "snapshot frozen at $snap"
  exit 0
fi

tag="${1:?usage: tools/check-overlay.sh <tag> <path under ess/>...}"
shift
[ -d "$snap" ] || { echo "no snapshot; run tools/check-overlay.sh --freeze" >&2; exit 2; }
[ -f "$snap/cloud/cloud.lock" ] ||
  { echo "the snapshot has no vendored files (cloud/cloud.lock); run tools/check-overlay.sh --freeze" >&2; exit 2; }
for p in "$@"; do
  case $p in
    cloud/*|./cloud/*)
      echo "$p is vendored: change cloudspec, then task vendor-update REV=<commit>, then tools/check-overlay.sh --freeze" >&2
      exit 2 ;;
  esac
done

dir="$root/.check/overlay-$tag"
rm -rf "$dir"
mkdir -p "$dir"
cp -r "$snap/." "$dir/"
for p in "$@"; do
  mkdir -p "$dir/$(dirname "$p")"
  cp "$root/ess/$p" "$dir/$p"
done

ess specify validate --path "$dir" --strict-requires
