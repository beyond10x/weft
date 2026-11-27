// Adversary cases from the round that found L2 switching the theme in place (a7cccc3): L2 then
// loaded each page once per gating viewport and switched to the other theme in place, so a defect
// seen only on a fresh load in one theme at one size passed. L2 now loads each page fresh in each
// theme at each size; these cases hold that. The run below is site.spec.ts itself, unchanged, against a
// built site of two fixture pages (`ux/fixtures/theme-site/`, served through `UX_BUILD`), each with a
// defect a reader meets on a fresh load in the light theme at 1280x800:
//
// - `theme-load-only`: a part drawn once at load for the theme then shown (wider than the screen in
//   the light theme on a desktop screen): gate 1 on a fresh load, nothing after a switch;
// - `theme-end`: the page is longer in the light theme, and at its true end the docs sidebar has
//   scrolled away: gate 2 at the end of a fresh load; after a switch the "end" read stays where the
//   dark page ended.
//
// Every hard gate is to run at both gating viewports in both themes (story docs-ux-budget,
// acceptance 3), so each defect must fail L2.

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { OUT, PORT, SITE, UX } from './adapter.ts';

// One run of L2 for both cases, in one worker; a failing case does not skip the other.
test.describe.configure({ mode: 'default' });

const RUN_OUT = join(OUT, 'theme-switch-adversary');
let output = '';
let status: number | null = null;

test.beforeAll(async () => {
  test.setTimeout(60_000);
  rmSync(RUN_OUT, { recursive: true, force: true });
  const run = spawnSync(
    process.execPath,
    [join(SITE, 'node_modules', '@playwright', 'test', 'cli.js'), 'test', 'ux/site.spec.ts', '--grep', 'L2 theme-', '--workers=1', '--reporter=line', '--output', join(RUN_OUT, 'test-results')],
    {
      cwd: SITE,
      encoding: 'utf8',
      timeout: 55_000,
      env: { ...process.env, UX_BUILD: join(UX, 'fixtures', 'theme-site'), UX_PORT: String(PORT + 71), UX_OUT: RUN_OUT, UX_RATCHET: '0' },
    },
  );
  status = run.status;
  output = `${run.stdout}${run.stderr}`;
});

/** The gating lines L2 wrote for a fixture page, over both themes. */
function gating(page: string): string[] {
  const lines: string[] = [];
  for (const theme of ['light', 'dark']) {
    const file = join(RUN_OUT, 'pages', `${page}--${theme}.json`);
    expect(existsSync(file), `L2 ran ${page} (${file}); run output:\n${output}`).toBe(true);
    lines.push(...(JSON.parse(readFileSync(file, 'utf8')) as { gating: string[] }).gating);
  }
  return lines;
}

test('L2 fails a page drawn wider than the screen on a fresh load in the light theme at 1280x800', () => {
  const lines = gating('theme-load-only');
  expect(lines.filter((l) => /^1280x800\b.*: gate 1 /.test(l)), `gating lines: ${JSON.stringify(lines)}`).not.toEqual([]);
  expect(status, output).not.toBe(0);
});

test('L2 fails a page whose docs sidebar is gone at the true end of the light page at 1280x800', () => {
  const lines = gating('theme-end');
  expect(lines.filter((l) => /^1280x800\b.*end: gate 2 docs sidebar /.test(l)), `gating lines: ${JSON.stringify(lines)}`).not.toEqual([]);
  expect(status, output).not.toBe(0);
});
