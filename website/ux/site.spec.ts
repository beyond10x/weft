// L2, the layout contract, docs variant: every sitemap page in each theme, loaded fresh at each
// gating viewport and read at its top and at its end: hard gate 1 (no horizontal page scroll,
// nothing past its box or cut off), gate 2 (the always-visible elements fully in view) and gate 5
// (axe-core: no serious or critical violation). The measured-only viewports are read and recorded
// at the top of the 1280x800 page resized, in the light theme, not gated. The page's moderate and
// minor axe nodes (the worse viewport) go to the ratchet as `page.<name>`.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { BASE, GATING, MEASURED, NOT_READY, ORIGIN, OUT, THEMES, alwaysVisible, sitemapPages } from './adapter.ts';
import type { Theme, Viewport } from './adapter.ts';
import { MEASURE } from './reporter.ts';
import { assertTheme, axe, hermetic, layoutFailures, nodesOf, prepare, quiet, serious, settle, visibilityFailures } from './page.ts';
import type { AxeViolation } from './page.ts';

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

/** Gates 1 and 2 at the page's current scroll position. */
async function read(page: Page, viewport: Viewport, state: string): Promise<string[]> {
  return [
    ...(await layoutFailures(page)).map((f) => `${viewport.name} ${state}: gate 1 ${f}`),
    ...(await visibilityFailures(page, alwaysVisible(viewport))).map((f) => `${viewport.name} ${state}: gate 2 ${f}`),
  ];
}

/** The page loaded fresh at one gating viewport: gate 5, then gates 1 and 2 at its top and its end. */
async function gatesAt(page: Page, path: string, viewport: Viewport, theme: Theme): Promise<{ failures: string[]; violations: AxeViolation[] }> {
  await prepare(page, viewport, theme);
  const response = await page.goto(`${BASE}${path}`);
  expect(response?.status(), `${path} answers`).toBe(200);
  await settle(page, NOT_READY);
  await assertTheme(page, theme);
  const violations = await axe(page);
  const failures = serious(violations).map(
    (v) => `${viewport.name}: gate 5 ${v.impact} ${v.id} (${v.help}) at ${v.nodes.length} node(s): ${v.nodes.slice(0, 3).join(', ')}`,
  );
  failures.push(...(await read(page, viewport, 'top')));
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await quiet(page);
  failures.push(...(await read(page, viewport, 'end')));
  return { failures, violations };
}

for (const site of sitemapPages()) {
  for (const theme of THEMES) {
    test(`L2 ${site.name} ${theme.name}`, async ({ page }) => {
      const gating: string[] = [];
      const axeBy: Record<string, AxeViolation[]> = {};
      const measured: Record<string, string[]> = {};
      for (const viewport of GATING) {
        const { failures, violations } = await gatesAt(page, site.path, viewport, theme);
        gating.push(...failures);
        axeBy[viewport.name] = violations;
        // Measured-only viewports, recorded not gated: the desktop page resized, its top, in one
        // theme (layout does not change with the theme).
        if (viewport === GATING[0] && theme.name === THEMES[0]!.name) {
          for (const m of MEASURED) {
            await page.setViewportSize({ width: m.width, height: m.height });
            await page.evaluate(() => window.scrollTo(0, 0));
            await quiet(page);
            measured[m.name] = await read(page, m, 'top');
          }
        }
      }
      const worst = (impact: string) => Math.max(...Object.values(axeBy).map((v) => nodesOf(v, impact)));
      mkdirSync(join(OUT, 'pages'), { recursive: true });
      mkdirSync(MEASURE, { recursive: true });
      writeFileSync(join(OUT, 'pages', `${site.name}--${theme.name}.json`), JSON.stringify({ page: site.name, theme: theme.name, gating, measured, axe: axeBy }, null, 1));
      writeFileSync(join(MEASURE, `page-${site.name}--${theme.name}.json`), JSON.stringify({ [`page.${site.name}`]: { 'axe.moderate': worst('moderate'), 'axe.minor': worst('minor') } }));
      expect(gating).toEqual([]);
    });
  }
}
