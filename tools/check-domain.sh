#!/bin/sh
# Validate one domain together with weft.shared only, so domains can be written in parallel while
# another domain's file is half written. The domain file may sit in any layer directory under ess/.
# Usage: tools/check-domain.sh <name>
set -eu

name="${1:?usage: tools/check-domain.sh <domain name, e.g. accounts>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
src="$(find "$root/ess" -name "$name.yaml" -not -path '*/.check/*' | head -n 1)"
shared="$root/ess/0-foundation/shared.yaml"
dir="$root/.check/$name"

[ -n "$src" ] && [ -f "$src" ] || { echo "no domain file named $name.yaml under $root/ess" >&2; exit 2; }

rm -rf "$dir"
mkdir -p "$dir/domains"
cp "$shared" "$dir/domains/shared.yaml"
[ "$name" = shared ] || cp "$src" "$dir/domains/$name.yaml"

domains="  - weft.shared"
files="  - domains/shared.yaml"
if [ "$name" != shared ]; then
  domains="$domains
  - weft.$name"
  files="$files
  - domains/$name.yaml"
fi

printf 'format: ess/37\nsystem: weft\nversion: v1\nsummary: Check of weft.%s with weft.shared.\ndomains:\n%s\n' \
  "$name" "$domains" > "$dir/system.yaml"
printf 'format: ess-inputs/2\nrequires: ess 0.99.2\nspecification:\n  - system.yaml\n%s\nscenarios: []\n' \
  "$files" > "$dir/ess-inputs.yaml"

ess specify validate --path "$dir" --strict-requires
