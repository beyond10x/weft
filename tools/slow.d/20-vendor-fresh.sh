#!/bin/sh
# Slow hook: copy cloudspec's blocks afresh at the pin and compare with ess/cloud/.
#
# Usage: tools/slow.d/20-vendor-fresh.sh
#
# Fetches beyond10x/cloudspec at CLOUDSPEC_REV of Taskfile.yml (one commit, depth 1) into
# target/cloudspec-src and runs its `scripts/vendor.sh --check` on ess/cloud: every file against
# the lock, nothing unlisted, and a fresh run from the commit and the excludes the lock names gives
# the same lock and the same bytes. Exits with vendor.sh's code (1: ess/cloud differs), 2 when
# the pin cannot be read or the fetch fails. Runs when ess/cloud/** or CLOUDSPEC_REV changed: a
# run that passed writes the digest of both to target/vendor-fresh/passed, and the next run with
# the same digest passes without fetching. CI runs it in the job vendor-fresh of
# .github/workflows/gate.yml when the pull request or push changes either. CLOUDSPEC_URL names
# another remote (default https://github.com/beyond10x/cloudspec). Fetches from GitHub, so never
# part of task check; the offline check is tools/check.d/15-vendor-lock.sh.
set -u
LC_ALL=C
export LC_ALL

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 2
url=${CLOUDSPEC_URL:-https://github.com/beyond10x/cloudspec}
src=target/cloudspec-src
stamp=target/vendor-fresh/passed

rev=$(sed -n "s/^[[:space:]][[:space:]]*CLOUDSPEC_REV:[[:space:]]*[\"']\{0,1\}\([0-9a-f]\{40\}\)[\"']\{0,1\}[[:space:]]*\$/\1/p" Taskfile.yml)
case $rev in
  '' | *[!0-9a-f]*) echo "vendor-fresh: Taskfile.yml sets no CLOUDSPEC_REV of 40 hex digits" >&2; exit 2 ;;
esac

sha256() {
  if command -v sha256sum > /dev/null 2>&1; then sha256sum | cut -d' ' -f1
  else shasum -a 256 | cut -d' ' -f1; fi
}
# The digest of the pin and of every file under ess/cloud, with its path.
digest=$({
  echo "CLOUDSPEC_REV $rev"
  find ess/cloud -type f | sort | while IFS= read -r f; do
    printf '%s %s\n' "$(sha256 < "$f")" "$f"
  done
} | sha256)
if [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$digest" ]; then
  echo "vendor-fresh: ess/cloud and CLOUDSPEC_REV $rev are unchanged since a run that passed ($stamp); skipped"
  exit 0
fi

# A checkout of its own: git -C never reaches weft's repository from here.
[ -d "$src/.git" ] || { mkdir -p "$src" && git -C "$src" init -q; } || exit 2
if [ "$(git -C "$src" rev-parse -q --verify "$rev^{commit}" 2> /dev/null)" != "$rev" ]; then
  echo "vendor-fresh: fetching $url at $rev"
  git -C "$src" fetch -q --depth 1 "$url" "$rev" || { echo "vendor-fresh: could not fetch $url at $rev" >&2; exit 2; }
fi
git -C "$src" -c advice.detachedHead=false checkout -q --force "$rev" || exit 2

rm -f "$stamp"
"$src/scripts/vendor.sh" --check "$root/ess/cloud"
rc=$?
if [ "$rc" = 0 ]; then
  mkdir -p "$(dirname "$stamp")" && printf '%s\n' "$digest" > "$stamp"
else
  echo "vendor-fresh: ess/cloud differs from cloudspec's vendor.sh at $rev (exit $rc); task vendor-update REV=$rev copies it again" >&2
fi
exit "$rc"
