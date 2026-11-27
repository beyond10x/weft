// L2, the layout contract, docs variant: every sitemap page in each theme, read at each gating
// viewport at its top and at its end: hard gate 1 (no horizontal page scroll, nothing past its box)
// and gate 2 (the always-visible elements fully in view). Gate 5: axe-core reports no serious or
// critical violation at 1280x800. The measured-only viewports are read and recorded in the light
// theme, not gated. The page's moderate and minor axe nodes go to the ratchet as `page.<name>`.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { BASE, GATING, MEASURED, NOT_READY, ORIGIN, OUT, THEMES, alwaysVisible, sitemapPages } from './adapter.ts';
import type { Viewport } from './adapter.ts';
import { MEASURE } from './reporter.ts';
import { assertTheme, axe, hermetic, layoutFailures, nodesOf, prepare, quiet, serious, settle, visibilityFailures } from './page.ts';
import type { Page } from '@playwright/test';

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

/** Gates 1 and 2 at one viewport, at the page's top and at its end. */
async function layoutAt(page: Page, viewport: Viewport): Promise<string[]> {
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await page.evaluate(() => window.scrollTo(0, 0));
  await quiet(page);
  const failures: string[] = [];
  const read = async (state: string) => {
    for (const f of await layoutFailures(page)) failures.push(`${viewport.name} ${state}: gate 1 ${f}`);
    for (const f of await visibilityFailures(page, alwaysVisible(viewport))) failures.push(`${viewport.name} ${state}: gate 2 ${f}`);
  };
  await read('top');
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await quiet(page);
  await read('end');
  return failures;
}

for (const site of sitemapPages()) {
  for (const theme of THEMES) {
    test(`L2 ${site.name} ${theme.name}`, async ({ page }) => {
      const first = GATING[0]!;
      await prepare(page, first, theme);
      const response = await page.goto(`${BASE}${site.path}`);
      expect(response?.status(), `${site.path} answers`).toBe(200);
      await settle(page, NOT_READY);
      await assertTheme(page, theme);
      const violations = await axe(page);
      const gating: string[] = [];
      for (const viewport of GATING) gating.push(...(await layoutAt(page, viewport)));
      const measured: Record<string, string[]> = {};
      // Measured-only viewports are recorded in one theme: layout does not change with the theme.
      if (theme.name === THEMES[0]!.name) for (const viewport of MEASURED) measured[viewport.name] = await layoutAt(page, viewport);
      const record = { page: site.name, theme: theme.name, gating, measured, axe: violations };
      mkdirSync(join(OUT, 'pages'), { recursive: true });
      mkdirSync(MEASURE, { recursive: true });
      writeFileSync(join(OUT, 'pages', `${site.name}--${theme.name}.json`), JSON.stringify(record, null, 1));
      writeFileSync(
        join(MEASURE, `page-${site.name}--${theme.name}.json`),
        JSON.stringify({ [`page.${site.name}`]: { 'axe.moderate': nodesOf(violations, 'moderate'), 'axe.minor': nodesOf(violations, 'minor') } }),
      );
      const gate5 = serious(violations).map((v) => `gate 5 ${v.impact} ${v.id} (${v.help}) at ${v.nodes.length} node(s): ${v.nodes.slice(0, 3).join(', ')}`);
      expect([...gating, ...gate5]).toEqual([]);
    });
  }
}
