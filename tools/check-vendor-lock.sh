#!/bin/sh
# Usage: tools/check-vendor-lock.sh [<root>]     (default: the repository root)
#        tools/check-vendor-lock.sh --excludes   print the excludes of D-a, one per line
#
# The offline check of the vendored cloudspec blocks (story cloud-blocks-lock-gate): the lock
# checks of cloudspec's `scripts/vendor.sh --check` at the pin, without git and without network,
# plus weft's own list of inputs. On <root>/ess/cloud/cloud.lock it refuses:
# - a line of a kind the format does not have, or a path of the wrong shape (`..`, `/`);
# - a format other than cloudspec-vendor/2, or not exactly one format line;
# - a system other than weft;
# - a lock made from a working tree, or a commit other than CLOUDSPEC_REV of <root>/Taskfile.yml;
# - exclude lines other than the excludes of D-a (13-actors/actors.yaml, components.yaml: weft
#   keeps its own actors and components), or an exclude the lock also lists as an output;
# - an output without its source line;
# - an output that is missing, or whose SHA-256 differs from the lock;
# - a YAML file (.yaml, .yml) under ess/cloud/ that the lock does not list as an output;
# - an output that the specification list of <root>/ess/ess-inputs.yaml does not name as
#   cloud/<output>.
# Prints each problem on standard error and exits 1 when there is one, 0 otherwise, 2 when <root>
# is not a directory. The slow lane (tools/slow.d/20-vendor-fresh.sh) re-runs the copy from source.
set -uf
LC_ALL=C
export LC_ALL

# The excludes of D-a (epic:cloud-blocks), sorted as vendor.sh writes them; task vendor-update
# passes them to vendor.sh.
excludes_d_a='13-actors/actors.yaml
components.yaml'
if [ "${1:-}" = --excludes ]; then
  printf '%s\n' "$excludes_d_a"
  exit 0
fi

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}
cd "$root" 2> /dev/null || { echo "vendor-lock: no directory $root" >&2; exit 2; }
cloud=ess/cloud
lock=$cloud/cloud.lock
inputs=ess/ess-inputs.yaml
nl='
'
IFS=$nl
bad=0
say() { echo "vendor-lock: $*" >&2; bad=1; }
# words <word>...: the words joined by one space.
words() { (IFS=' '; printf '%s' "$*"); }

sha256() {
  if command -v sha256sum > /dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

[ -f "$lock" ] || { echo "vendor-lock: no $lock in $root" >&2; exit 1; }

# Every line is of a kind cloudspec-vendor/2 has, with paths of the shape vendor.sh writes.
path='([0-9]+-[a-z0-9-]+/[a-z0-9_-]+|components)\.yaml'
bad_lines=$(grep -nvE \
  -e '^format [^ ]+$' \
  -e '^(commit|worktree) [0-9a-f]{40}$' \
  -e '^system [a-z][a-z0-9_]*$' \
  -e "^exclude $path\$" \
  -e "^source [0-9a-f]{64} blocks/$path\$" \
  -e "^output [0-9a-f]{64} $path\$" "$lock")
for l in $bad_lines; do
  say "$lock:${l%%:*}: a line the format cloudspec-vendor/2 does not have: ${l#*:}"
done

formats=$(sed -n 's/^format //p' "$lock")
case $formats in
  '') say "$lock has no format line" ;;
  *"$nl"*) say "$lock has more than one format line: $(words $formats)" ;;
  cloudspec-vendor/2) ;;
  *) say "$lock is format $formats, not cloudspec-vendor/2: vendor again (task vendor-update)" ;;
esac

systems=$(sed -n 's/^system //p' "$lock")
case $systems in
  weft) ;;
  '') say "$lock names no system" ;;
  *) say "$lock names system $(words $systems), not weft" ;;
esac

if grep -q '^worktree ' "$lock"; then
  say "$lock was made from a working tree of cloudspec, not from a commit: vendor again (task vendor-update)"
fi
commits=$(sed -n 's/^commit //p' "$lock")
pins=$(sed -n "s/^[[:space:]][[:space:]]*CLOUDSPEC_REV:[[:space:]]*[\"']\{0,1\}\([0-9a-f][0-9a-f]*\)[\"']\{0,1\}[[:space:]]*\$/\1/p" Taskfile.yml 2> /dev/null)
case $pins in
  '') say "Taskfile.yml sets no CLOUDSPEC_REV" ;;
  *"$nl"*) say "Taskfile.yml sets CLOUDSPEC_REV more than once" ;;
  *)
    case $commits in
      '') say "$lock names no commit" ;;
      "$pins") ;;
      *) say "$lock names commit $(words $commits) and CLOUDSPEC_REV in Taskfile.yml is $pins" ;;
    esac ;;
esac

excludes=$(sed -n 's/^exclude //p' "$lock" | sort)
[ "$excludes" = "$excludes_d_a" ] ||
  say "$lock excludes '$(words $excludes)', not the excludes of D-a: $(words $excludes_d_a)"

outputs=$(sed -n 's/^output [0-9a-f]* //p' "$lock")
sources=$(sed -n 's/^source [0-9a-f]* //p' "$lock")
# The specification list of ess-inputs.yaml: the items under `specification:` up to the next
# top-level key; comments and quotes are not part of an item.
listed=$(awk '
  /^specification:[[:space:]]*$/ { spec = 1; next }
  /^[^[:space:]#-]/ { spec = 0 }
  /^[[:space:]]*#/ { next }
  spec && /^[[:space:]]*-[[:space:]]/ {
    sub(/^[[:space:]]*-[[:space:]]+/, ""); sub(/[[:space:]]+#.*$/, ""); sub(/[[:space:]]+$/, "")
    gsub(/["\047]/, ""); print
  }' "$inputs" 2> /dev/null) || say "no $inputs in $root"

for x in $excludes; do
  printf '%s\n' "$outputs" | grep -qxF -- "$x" && say "$lock excludes $x and lists it as an output"
done

for d in $(sed -n 's/^output //p' "$lock"); do
  digest=${d%% *}
  f=${d#* }
  printf '%s\n' "$sources" | grep -qxF -- "blocks/$f" || say "$lock lists output $f without its source line blocks/$f"
  case $f in *..* | /*) continue ;; esac
  if [ ! -f "$cloud/$f" ]; then
    say "$f is listed in the lock and missing from $cloud"
  elif [ "$(sha256 "$cloud/$f")" != "$digest" ]; then
    say "$cloud/$f differs from its digest in the lock: vendor again (task vendor-update), never edit a vendored file"
  fi
  printf '%s\n' "$listed" | grep -qxF -- "cloud/$f" ||
    say "$f is an output of the lock and $inputs does not list cloud/$f under specification"
done

present=$(cd "$cloud" && find . -type f \( -name '*.yaml' -o -name '*.yml' \) | sed 's#^\./##' | sort)
for f in $present; do
  printf '%s\n' "$outputs" | grep -qxF -- "$f" || say "not in the lock: $f (under $cloud)"
done

[ "$bad" = 0 ] || exit 1
echo "vendor-lock: $cloud matches $lock (format cloudspec-vendor/2, system weft, commit $commits = CLOUDSPEC_REV, $(printf '%s\n' "$outputs" | grep -c .) outputs listed in $inputs)"
exit 0
