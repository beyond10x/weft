// The harness's self-test: each check fails where it must, so a green run means something. The
// layout check fails on a page with an element 200 px wider than the viewport, the accessibility
// check fails on a page with a serious violation (`ux/fixtures/`, served outside the site), and the
// ratchet CLI exits 1 on a worse value, on an improvement not written in and on a missing value.

import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { GATING, ORIGIN, OUT, UX } from './adapter.ts';
import { axe, hermetic, layoutFailures, serious, visibilityFailures } from './page.ts';
import { ABOUT, TOLERANCES } from './ratchet.ts';
import type { Baseline, Measured } from './ratchet.ts';

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

for (const viewport of GATING) {
  test(`self-test: the layout check fails on the overflow fixture at ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.goto(`${ORIGIN}/__ux/fixtures/overflow.html`);
    const failures = await layoutFailures(page);
    expect(failures.some((f) => f.startsWith('document') && f.endsWith('past by 200px'))).toBe(true);
    expect(failures.some((f) => f.startsWith('viewport div#wide'))).toBe(true);
  });
}

test('self-test: the always-visible check fails on an element cut off by its ancestor clip, outside it or covered', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 300 });
  await page.setContent(
    '<body style="margin:0">' +
      '<div style="clip-path:inset(0);height:100px"><nav id="clipped" style="position:fixed;top:0;bottom:0;width:100px">menu</nav></div>' +
      '<button id="out" style="position:absolute;top:290px;left:200px">out</button>' +
      '<button id="covered" style="position:absolute;top:150px;left:200px">covered</button><div style="position:absolute;top:140px;left:190px;width:120px;height:40px;background:#000"></div>' +
      '<button id="shown" style="position:absolute;top:220px;left:200px">shown</button>' +
      '</body>',
  );
  const failures = await visibilityFailures(page, ['clipped', 'out', 'covered', 'shown', 'absent'].map((id) => ({ name: id, selector: `#${id}` })));
  expect(failures.map((f) => f.split(' ')[0])).toEqual(['clipped', 'out', 'covered', 'absent']);
});

test('self-test: the accessibility check fails on the serious-violation fixture', async ({ page }) => {
  await page.goto(`${ORIGIN}/__ux/fixtures/axe-serious.html`);
  const found = serious(await axe(page));
  expect(found.map((v) => `${v.impact} ${v.id}`).sort()).toEqual(['critical image-alt', 'serious color-contrast']);
});

const baseline: Baseline = {
  about: ABOUT,
  scenarios: {
    validate: {
      'screen_only.actions': { value: 3, tolerance: 0 },
      'screen_only.looks': { value: 4, tolerance: 0 },
      layout_shift: { value: 0.02, tolerance: 0.01 },
    },
    'page.docs': { 'axe.moderate': { value: 2, tolerance: 0 }, 'axe.minor': { value: 0, tolerance: 0 } },
  },
  tolerances: TOLERANCES,
};

const same: Measured = {
  validate: { 'screen_only.actions': 3, 'screen_only.looks': 4, layout_shift: 0.02 },
  'page.docs': { 'axe.moderate': 2, 'axe.minor': 0 },
};

function ratchet(name: string, measured: Measured): { status: number | null; out: string } {
  const dir = join(OUT, 'self-test', name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'baseline.json'), JSON.stringify(baseline));
  writeFileSync(join(dir, 'measured.json'), JSON.stringify(measured));
  const run = spawnSync(process.execPath, [join(UX, 'ratchet.ts'), 'check', '--baseline', join(dir, 'baseline.json'), '--measured', join(dir, 'measured.json')], {
    encoding: 'utf8',
  });
  return { status: run.status, out: `${run.stdout}${run.stderr}` };
}

const cases: { name: string; measured: Measured; status: number; says: string | null }[] = [
  { name: 'equal', measured: same, status: 0, says: null },
  { name: 'within-tolerance', measured: { ...same, validate: { ...same.validate, layout_shift: 0.025 } }, status: 0, says: null },
  { name: 'worse', measured: { ...same, validate: { ...same.validate, 'screen_only.actions': 4 } }, status: 1, says: 'worse validate screen_only.actions' },
  { name: 'worse-shift', measured: { ...same, validate: { ...same.validate, layout_shift: 0.04 } }, status: 1, says: 'worse validate layout_shift' },
  { name: 'improved', measured: { ...same, 'page.docs': { 'axe.moderate': 1, 'axe.minor': 0 } }, status: 1, says: 'improved-not-taken page.docs axe.moderate' },
  { name: 'missing-measured', measured: { ...same, validate: { 'screen_only.actions': 3, 'screen_only.looks': 4 } }, status: 1, says: 'missing validate layout_shift' },
  { name: 'missing-baseline', measured: { ...same, replay: { 'screen_only.actions': 2 } }, status: 1, says: 'missing replay screen_only.actions' },
];

for (const c of cases) {
  test(`self-test: the ratchet exits ${c.status} on ${c.name}`, () => {
    const run = ratchet(c.name, c.measured);
    expect(run.status, run.out).toBe(c.status);
    if (c.says) expect(run.out).toContain(`ratchet: ${c.says}`);
  });
}
