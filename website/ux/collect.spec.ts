// The run collects the specs under `ux/` and none under `ux/.out/` (`playwright.config.ts`,
// `testIgnore`): a probe or a copy left there by an earlier run would otherwise run with the step,
// with its cost and its result.

import { spawnSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { SITE, UX } from './adapter.ts';

test('the run collects no spec under ux/.out', () => {
  const dir = join(UX, '.out', 'collect-check');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'stray.spec.ts'), "import { test } from '@playwright/test';\ntest('stray spec under ux/.out', () => {});\n");
  try {
    const run = spawnSync(process.execPath, [join(SITE, 'node_modules', '@playwright', 'test', 'cli.js'), 'test', '--list'], {
      cwd: SITE,
      encoding: 'utf8',
      timeout: 30_000,
    });
    const output = `${run.stdout}${run.stderr}`;
    expect(run.status, output).toBe(0);
    expect(output, 'the run lists this spec').toContain('the run collects no spec under ux/.out');
    expect(output).not.toContain('stray spec under ux/.out');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
