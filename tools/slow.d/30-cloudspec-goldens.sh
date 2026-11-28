#!/bin/sh
# Slow hook: compare cloudspec's weft goldens in tests/fixtures/cloudspec/ with cloudspec's own
# files at the commit goldens.lock names.
#
# Usage: tools/slow.d/30-cloudspec-goldens.sh
#
# goldens.lock is one line `commit <40 hex>`, then one line `<sha256> <file>` per golden. Fetches
# beyond10x/cloudspec at that commit (one commit, depth 1) into a checkout of its own,
# target/cloudspec-goldens-src, and compares the bytes of each golden with
# fixtures/weft/<file> there; it also checks each lock line against the file in the tree. It
# builds nothing. Exits 1 when a golden differs, 2 when the lock cannot be read or the fetch
# fails. Runs when goldens.lock or a golden changed: a run that passed writes the digest of the
# sha256 of goldens.lock and of each golden, with its path, to target/cloudspec-goldens/passed,
# and the next run with the same digest skips without fetching; a run that fails removes it.
# CLOUDSPEC_URL names another remote (default https://github.com/beyond10x/cloudspec). Fetches,
# so never part of task check; the test step checks the goldens offline against the lock
# (crate weft-cloudspec, tests/goldens.rs).
set -u
LC_ALL=C
export LC_ALL

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 2
url=${CLOUDSPEC_URL:-https://github.com/beyond10x/cloudspec}
src=target/cloudspec-goldens-src
stamp=target/cloudspec-goldens/passed
dir=tests/fixtures/cloudspec
lock=$dir/goldens.lock

[ -f "$lock" ] || { echo "cloudspec-goldens: no $lock" >&2; exit 2; }
rev=$(sed -n '1s/^commit \([0-9a-f]\{40\}\)$/\1/p' "$lock")
[ -n "$rev" ] || { echo "cloudspec-goldens: the first line of $lock is not \`commit <40 hex>\`" >&2; exit 2; }
files=$(sed -n '2,$p' "$lock" | cut -d' ' -f2-)
[ -n "$files" ] || { echo "cloudspec-goldens: $lock names no golden" >&2; exit 2; }
bad=$(sed -n '2,$p' "$lock" | grep -v '^[0-9a-f]\{64\} [A-Za-z0-9._-][A-Za-z0-9._-]*$')
[ -z "$bad" ] || { echo "cloudspec-goldens: $lock: not \`<sha256> <file>\`: $bad" >&2; exit 2; }

sha256() {
  if command -v sha256sum > /dev/null 2>&1; then sha256sum | cut -d' ' -f1
  else shasum -a 256 | cut -d' ' -f1; fi
}
# The digest of goldens.lock and of every golden it names, with its path.
digest=$({
  printf '%s %s\n' "$(sha256 < "$lock")" "$lock"
  echo "$files" | while IFS= read -r f; do
    if [ -f "$dir/$f" ]; then printf '%s %s\n' "$(sha256 < "$dir/$f")" "$dir/$f"
    else printf 'missing %s\n' "$dir/$f"; fi
  done
} | sha256)
if [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$digest" ]; then
  echo "cloudspec-goldens: $dir is unchanged since a run that passed at cloudspec $rev ($stamp); skipped"
  exit 0
fi
rm -f "$stamp"

# A checkout of its own: git -C never reaches weft's repository from here.
[ -d "$src/.git" ] || { mkdir -p "$src" && git -C "$src" init -q; } || exit 2
if [ "$(git -C "$src" rev-parse -q --verify "$rev^{commit}" 2> /dev/null)" != "$rev" ]; then
  echo "cloudspec-goldens: fetching $url at $rev"
  git -C "$src" fetch -q --depth 1 "$url" "$rev" || { echo "cloudspec-goldens: could not fetch $url at $rev" >&2; exit 2; }
fi

rc=0
sed -n '2,$p' "$lock" | {
  fail=0
  while IFS=' ' read -r want f; do
    if [ ! -f "$dir/$f" ]; then
      echo "cloudspec-goldens: $dir/$f is missing" >&2; fail=1; continue
    fi
    if [ "$(sha256 < "$dir/$f")" != "$want" ]; then
      echo "cloudspec-goldens: $dir/$f does not match its line of $lock" >&2; fail=1
    fi
    if ! git -C "$src" cat-file -e "$rev:fixtures/weft/$f" 2> /dev/null; then
      echo "cloudspec-goldens: cloudspec $rev has no fixtures/weft/$f" >&2; fail=1; continue
    fi
    if git -C "$src" cat-file blob "$rev:fixtures/weft/$f" | cmp -s - "$dir/$f"; then
      echo "cloudspec-goldens: $dir/$f equals cloudspec $rev:fixtures/weft/$f"
    else
      echo "cloudspec-goldens: $dir/$f differs from cloudspec $rev:fixtures/weft/$f" >&2; fail=1
    fi
  done
  exit "$fail"
} || rc=1

if [ "$rc" = 0 ]; then
  mkdir -p "$(dirname "$stamp")" && printf '%s\n' "$digest" > "$stamp"
else
  echo "cloudspec-goldens: copy the goldens again from cloudspec $rev and write goldens.lock anew" >&2
fi
exit "$rc"
