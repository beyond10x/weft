#!/bin/sh
# The two consoles of product/console (docs/run/consoles.md in website/docs): both ESS UI documents
# against the model, including the host components the renderer registers; then the renderer's
# type check, unit tests, one build, and the Playwright run on that build (every page of both
# apps loads, no axe violation, no horizontal overflow at 390 px). Budget: each test 60 s
# (vitest testTimeout, Playwright timeout); the test step (type check to Playwright) is killed at
# 120 s. npm ci runs only when package-lock.json changed; in CI the browser is installed first.
set -eu
console=product/console
stamp=target/console/npm-ci.sha256

for app in operator account; do
  ess ui check --model ess --path "$console/ui/$app.ui.yaml" --hosts "$console/ui/$app.hosts"
done

command -v node > /dev/null 2>&1 || { echo "40-console: node is not on PATH (node 22 or later)" >&2; exit 1; }
mkdir -p target/console
want=$(sha256sum "$console/package-lock.json" | cut -d' ' -f1)
have=$(cat "$stamp" 2> /dev/null || true)
if [ ! -d "$console/node_modules" ] || [ "$want" != "$have" ]; then
  npm --prefix "$console" ci --no-audit --no-fund
  echo "$want" > "$stamp"
fi
if [ "${CI:-}" = true ]; then
  (cd "$console" && npx playwright install --with-deps --only-shell chromium)
else
  (cd "$console" && npx playwright install --only-shell chromium)
fi

start=$(date +%s)
cd "$console"
timeout 120 sh -c 'npm run --silent typecheck && npm run --silent test && npm run --silent build > /dev/null && npm run --silent e2e' || {
  rc=$?
  [ "$rc" = 124 ] && echo "40-console: the test step ran over 120 s" >&2
  exit "$rc"
}
echo "40-console: test step $(( $(date +%s) - start )) s"
