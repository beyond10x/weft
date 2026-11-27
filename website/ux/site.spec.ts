// L2, the layout contract, docs variant: every sitemap page, loaded fresh at each gating viewport in
// each theme and read at its top and at its end: hard gate 1 (no horizontal page scroll, nothing
// past its box or cut off), gate 2 (the always-visible elements fully in view) and gate 5
// (axe-core: no serious or critical violation). One test per page, four fresh loads: no theme is
// switched in place, since a part drawn once at load for the theme then shown would be missed
// (`theme-switch-adversary.spec.ts`). The measured-only viewports are read and recorded at the top
// of the 1280x800 page resized, in the light theme, not gated. The page's moderate and minor axe
// nodes (the worse viewport) go to the ratchet as `page.<name>`, one fragment per theme.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test as base } from '@playwright/test';
import type { BrowserContext, Page } from '@playwright/test';
import { BASE, GATING, MEASURED, NOT_READY, ORIGIN, OUT, THEMES, alwaysVisible, sitemapPages } from './adapter.ts';
import type { Required, Theme, Viewport } from './adapter.ts';
import { MEASURE } from './reporter.ts';
import { assertTheme, axe, gatesHere, hermetic, layoutFailures, nodesOf, prepare, serious, settle, visibilityFailures } from './page.ts';
import type { AxeViolation } from './page.ts';

/**
 * One browser tab per worker, with the project's context options, that loads every page of the
 * sitemap the worker runs, as a reader goes from page to page: the browser keeps what it compiled
 * of the site's scripts and of axe-core between the pages instead of starting a new browser
 * process per test. Every page is loaded fresh (`goto`) at each gating viewport, and every document
 * starts with its storage cleared, as in a new context, so no page sees a theme or a search another
 * page left behind.
 */
const test = base.extend<{ page: Page }, { tab: Page }>({
  tab: [
    async ({ browser }, use, workerInfo) => {
      const { deviceScaleFactor, locale, timezoneId } = workerInfo.project.use;
      const context: BrowserContext = await browser.newContext({ deviceScaleFactor, locale, timezoneId });
      await hermetic(context, ORIGIN);
      await context.addInitScript(() => {
        try {
          window.localStorage.clear();
          window.sessionStorage.clear();
        } catch {
          // a document without storage (about:blank) has nothing to clear
        }
      });
      await use(await context.newPage());
      await context.close();
    },
    { scope: 'worker' },
  ],
  page: async ({ tab }, use) => use(tab),
});

/** Gates 1 and 2, at the page's current scroll position or after scrolling to one of its ends. */
async function read(page: Page, where: string, viewport: Viewport, state: 'top' | 'end', scroll: boolean): Promise<string[]> {
  const { layout, visibility } = await gatesHere(page, alwaysVisible(viewport), scroll ? state : null);
  return [...layout.map((f) => `${where} ${state}: gate 1 ${f}`), ...visibility.map((f) => `${where} ${state}: gate 2 ${f}`)];
}

/** The page loaded fresh at one gating viewport in one theme: gate 5, then gates 1 and 2 at its top and at its end. */
async function gatesAt(page: Page, path: string, viewport: Viewport, theme: Theme): Promise<{ failures: string[]; violations: AxeViolation[] }> {
  const where = `${viewport.name} ${theme.name}`;
  await prepare(page, viewport, theme);
  const response = await page.goto(`${BASE}${path}`);
  expect(response?.status(), `${path} answers`).toBe(200);
  await settle(page, NOT_READY);
  await assertTheme(page, theme);
  const violations = await axe(page);
  const failures = serious(violations).map(
    (v) => `${where}: gate 5 ${v.impact} ${v.id} (${v.help}) at ${v.nodes.length} node(s): ${v.nodes.slice(0, 3).join(', ')}`,
  );
  failures.push(...(await read(page, where, viewport, 'top', false)));
  failures.push(...(await read(page, where, viewport, 'end', true)));
  return { failures, violations };
}

// The one-call read (`gatesHere`) against the checks the harness self-tests hold (`layoutFailures`,
// `visibilityFailures`): the same lines on pages that fail each gate and on one that passes.
const SHAPES: { name: string; width: number; height: number; html: string; required: Required[] }[] = [
  {
    name: 'text spilling over the next block and a command cut off',
    width: 390,
    height: 844,
    html:
      '<body style="margin:0;font:16px/20px sans-serif"><main>' +
      '<div id="short" style="height:20px;width:300px">first line of a paragraph that wraps over many lines and runs down over the next block of the page</div>' +
      '<p style="margin:0;width:300px">the next block, now written over</p>' +
      '<div id="narrow" style="width:120px;overflow:hidden;white-space:nowrap">ess specify validate --path ess --strict-requires</div>' +
      '</main></body>',
    required: [],
  },
  {
    name: 'elements clipped, outside, covered, under a pointer-events none overlay, faded, absent and shown',
    width: 400,
    height: 300,
    html:
      '<body style="margin:0">' +
      '<div style="clip-path:inset(0);height:100px"><nav id="clipped" style="position:fixed;top:0;bottom:0;width:100px">menu</nav></div>' +
      '<button id="out" style="position:absolute;top:290px;left:200px">out</button>' +
      '<button id="covered" style="position:absolute;top:150px;left:200px">covered</button><div style="position:absolute;top:140px;left:190px;width:120px;height:40px;background:#000"></div>' +
      '<button id="under" style="position:absolute;top:20px;left:300px">under</button><div style="position:absolute;top:0;left:290px;width:100px;height:60px;background:#000;pointer-events:none"></div>' +
      '<button id="faded" style="position:absolute;top:200px;left:20px;opacity:0">faded</button>' +
      '<button id="shown" style="position:absolute;top:220px;left:200px">shown</button>' +
      '</body>',
    required: ['clipped', 'out', 'covered', 'under', 'faded', 'shown', 'absent'].map((id) => ({ name: id, selector: `#${id}` })),
  },
  {
    name: 'a long page whose end holds a box past the right edge, and a marker that is not there',
    width: 390,
    height: 844,
    html:
      '<body style="margin:0;font:16px/20px sans-serif"><main><p style="height:3000px">top</p>' +
      '<div id="wide" style="width:600px">the end of the page, wider than the screen</div></main>' +
      '<button id="pinned" style="position:fixed;top:10px;left:10px">menu</button></body>',
    required: [
      { name: 'pinned', selector: '#pinned' },
      { name: 'only where marked', selector: '#nowhere', onlyWhere: '.no-such-marker' },
    ],
  },
];

for (const shape of SHAPES) {
  test(`L2 self-test: the one-call read gives what the separate checks give: ${shape.name}`, async ({ page }) => {
    await page.setViewportSize({ width: shape.width, height: shape.height });
    await page.setContent(shape.html);
    for (const scroll of [null, 'end', 'top'] as const) {
      const one = await gatesHere(page, shape.required, scroll);
      const at = await page.evaluate(() => Math.round(window.scrollY));
      if (scroll === 'top') expect(at, 'scrolled to the top').toBe(0);
      if (scroll === 'end') expect(at, 'scrolled to the end').toBe(await page.evaluate(() => Math.round(document.scrollingElement!.scrollHeight - document.scrollingElement!.clientHeight)));
      expect(one, `scroll ${scroll}`).toEqual({ layout: await layoutFailures(page), visibility: await visibilityFailures(page, shape.required) });
      expect([...one.layout, ...one.visibility], `scroll ${scroll}: the shape fails a gate`).not.toEqual([]);
    }
  });
}

test('L2 self-test: gate 5 runs every rule axe-core enables by default, not a WCAG tag list', async ({ page }) => {
  // Content outside any landmark and no level-one heading: best-practice rules only, in no WCAG tag.
  await page.setContent('<!doctype html><html lang="en"><head><title>t</title></head><body><div>text outside every landmark</div></body></html>');
  const ids = (await axe(page)).map((v) => v.id);
  expect(ids).toEqual(expect.arrayContaining(['region', 'page-has-heading-one']));
});

for (const site of sitemapPages()) {
  test(`L2 ${site.name}`, async ({ page }) => {
    const gating: string[] = [];
    const axeBy: Record<string, Record<string, AxeViolation[]>> = Object.fromEntries(THEMES.map((t) => [t.name, {}]));
    const measured: Record<string, string[]> = {};
    for (const viewport of GATING) {
      // The light theme last at 1280x800: the measured-only viewports resize that page.
      for (const theme of viewport === GATING[0] ? [...THEMES].reverse() : THEMES) {
        const { failures, violations } = await gatesAt(page, site.path, viewport, theme);
        gating.push(...failures);
        axeBy[theme.name]![viewport.name] = violations;
      }
      // Measured-only viewports, recorded not gated: the desktop page resized, its top, in one
      // theme (layout does not change with the theme).
      if (viewport === GATING[0]) {
        await assertTheme(page, THEMES[0]!);
        for (const m of MEASURED) {
          await page.setViewportSize({ width: m.width, height: m.height });
          measured[m.name] = await read(page, m.name, m, 'top', true);
        }
      }
    }
    mkdirSync(join(OUT, 'pages'), { recursive: true });
    mkdirSync(MEASURE, { recursive: true });
    for (const theme of THEMES) {
      const byViewport = axeBy[theme.name]!;
      const worst = (impact: string) => Math.max(...Object.values(byViewport).map((v) => nodesOf(v, impact)));
      const own = (line: string) => line.split(':')[0]!.split(' ')[1] === theme.name;
      writeFileSync(
        join(OUT, 'pages', `${site.name}--${theme.name}.json`),
        JSON.stringify({ page: site.name, theme: theme.name, gating: gating.filter(own), measured: theme === THEMES[0] ? measured : {}, axe: byViewport }, null, 1),
      );
      writeFileSync(join(MEASURE, `page-${site.name}--${theme.name}.json`), JSON.stringify({ [`page.${site.name}`]: { 'axe.moderate': worst('moderate'), 'axe.minor': worst('minor') } }));
    }
    expect(gating).toEqual([]);
  });
}
