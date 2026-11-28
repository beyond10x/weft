#!/bin/sh
# Validate one domain together with weft.shared and the vendored files only, so domains can be
# written in parallel while another domain's file is half written.
#
# The files validated, from ess/ess-inputs.yaml:
#   - every vendored file under ess/cloud/ (cloudspec's blocks, one layer below weft's own files,
#     changed only by `task vendor-update`): a domain of weft refers to them freely (weft.scope
#     exists only there), and a domain split over a vendored file and one of weft's own
#     (weft.workloads) is validated whole;
#   - ess/0-foundation/shared.yaml;
#   - the domain's own files, named <name>.yaml under a layer directory of ess/ outside
#     ess/cloud/ (a name only ess/cloud/ has, such as scope, validates the vendored files alone);
#   - ess/9-actors/actors.yaml with only the `may` items whose command those files declare, so
#     that a command reading `caller.<attribute>` has its caller;
#   - the `conversions:` block of ess/components.yaml, written as the components file.
# A domain that refers to another domain of weft's own files fails: use tools/check-overlay.sh.
# Usage: tools/check-domain.sh <name>
set -eu

name="${1:?usage: tools/check-domain.sh <domain name, e.g. accounts>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
inputs="$root/ess/ess-inputs.yaml"
actors=9-actors/actors.yaml
[ "$name" != actors ] ||
  { echo "check-domain: weft.actors names the commands of every domain; use tools/check-overlay.sh" >&2; exit 2; }
dir="$root/.check/$name"

files="$(sed -n 's#^  - \(\(cloud/\)\{0,1\}[0-9][0-9]*-[^/ ]*/[^/ ]*\.yaml\)$#\1#p' "$inputs" | while read -r path; do
  case $path in
    "$actors") ;;
    cloud/*|0-foundation/shared.yaml|*/"$name".yaml) printf '%s\n' "$path" ;;
  esac
done)"
printf '%s\n' "$files" | grep -q "^cloud/" ||
  { echo "check-domain: $inputs lists no vendored file under cloud/" >&2; exit 2; }
printf '%s\n' "$files" | grep -q "/$name\.yaml\$" ||
  { echo "no domain file named $name.yaml in $inputs" >&2; exit 2; }

rm -rf "$dir"
mkdir -p "$dir/${actors%/*}"
domains="
  - weft.actors"
listed="
  - components.yaml
  - $actors"
for path in $files; do
  mkdir -p "$dir/${path%/*}"
  cp "$root/ess/$path" "$dir/$path"
  listed="$listed
  - $path"
  d="$(sed -n 's/^domain: *//p' "$root/ess/$path" | head -n 1)"
  case "$domains
" in
    *"  - $d
"*) ;;
    *) domains="$domains
  - $d" ;;
  esac
  awk '/^[A-Za-z_]+:/ { sec = $1; next } sec == "commands:" && /^  - name:/ { print $3 }' \
    "$root/ess/$path" >> "$dir/commands.txt"
done

# the actors, each with only the commands of the files above
awk '
  NR == FNR { declared[$1] = 1; next }
  /^    [a-z_]+:/ { may = ($1 == "may:") }
  may && /^      - / && !($2 in declared) { next }
  { print }
' "$dir/commands.txt" "$root/ess/$actors" > "$dir/$actors"
awk '/^[a-z_]+:/ { on = ($1 == "conversions:") } on' "$root/ess/components.yaml" > "$dir/components.yaml"

printf 'format: ess/37\nsystem: weft\nversion: v1\nsummary: Check of %s with weft.shared and the vendored files.\ndomains:%s\n' \
  "$name" "$domains" > "$dir/system.yaml"
printf 'format: ess-inputs/2\nrequires: ess 0.99.2\nspecification:\n  - system.yaml%s\nscenarios: []\n' \
  "$listed" > "$dir/ess-inputs.yaml"

ess specify validate --path "$dir" --strict-requires
