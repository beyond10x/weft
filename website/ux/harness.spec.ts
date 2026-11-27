// The harness's self-test: each check fails where it must, so a green run means something. The
// layout check fails on a page with an element 200 px wider than the viewport, the accessibility
// check fails on a page with a serious violation (`ux/fixtures/`, served outside the site), and the
// ratchet CLI exits 1 on a worse value, on an improvement not written in, on a missing value and on
// a baseline entry whose tolerance is not the harness's own. The synthetic pages below each break one
// probe the way a page can hide content from a reader (cases first written by an adversary round).

import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { GATING, NAVIGATION, ORIGIN, OUT, UX } from './adapter.ts';
import { axe, hermetic, layoutFailures, serious, visibilityFailures } from './page.ts';
import { textGlance } from './probe.ts';
import { ABOUT, TOLERANCES } from './ratchet.ts';
import type { Baseline, Measured } from './ratchet.ts';

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

const MOBILE = GATING[1]!;

for (const viewport of GATING) {
  test(`self-test: the layout check fails on the overflow fixture at ${viewport.name}`, async ({ page }) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.goto(`${ORIGIN}/__ux/fixtures/overflow.html`);
    const failures = await layoutFailures(page);
    expect(failures.some((f) => f.startsWith('document') && f.endsWith('past by 200px'))).toBe(true);
    expect(failures.some((f) => f.startsWith('viewport div#wide'))).toBe(true);
  });
}

// Gate 1: content a reader cannot read.

test('self-test: the layout check fails on text spilling below its fixed-height box over the next block', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent(
    '<body style="margin:0;font:16px/20px sans-serif"><main>' +
      '<div id="short" style="height:20px;width:300px">first line of a paragraph that wraps over many lines and runs down over the next block of the page</div>' +
      '<p style="margin:0;width:300px">the next block, now written over</p>' +
      '</main></body>',
  );
  expect(await layoutFailures(page)).toEqual([expect.stringMatching(/^box div#short .* past by \d+px$/)]);
});

test('self-test: the layout check fails on content parked past the right edge under html overflow-x hidden', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent(
    '<html style="overflow-x:hidden"><body style="margin:0"><main><h1>Title</h1>' +
      '<p id="gone" style="position:relative;left:400px;width:300px">the answer the reader came for, out of reach to the right</p>' +
      '</main></body></html>',
  );
  expect(await layoutFailures(page)).not.toEqual([]);
});

test('self-test: the layout check fails on a command cut off by a narrow overflow-hidden box', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent(
    '<body style="margin:0"><main>' +
      '<div id="narrow" style="width:120px;overflow:hidden;white-space:nowrap">ess specify validate --path ess --strict-requires</div>' +
      '</main></body>',
  );
  expect(await layoutFailures(page)).toEqual([expect.stringMatching(/^cut div#narrow .* by \d+px$/)]);
});

test('self-test: the layout check passes text hidden for screen readers only and a box that scrolls', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent(
    '<body style="margin:0;font:16px/20px sans-serif"><main>' +
      '<a href="#x">Source<span style="position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0">(opens in new tab)</span></a>' +
      '<pre style="width:200px;overflow-x:auto">ess specify validate --path ess --strict-requires</pre>' +
      '<div style="height:40px;overflow-y:auto">one<br>two<br>three<br>four</div>' +
      '</main></body>',
  );
  expect(await layoutFailures(page)).toEqual([]);
});

// Gate 2: an always-visible element the reader cannot see.

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

test('self-test: the always-visible check fails on an element at opacity 0', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 300 });
  await page.setContent('<body style="margin:0"><button id="faded" style="position:absolute;top:20px;left:20px;opacity:0">search</button></body>');
  expect(await visibilityFailures(page, [{ name: 'faded', selector: '#faded' }])).toHaveLength(1);
});

test('self-test: the always-visible check fails on an element under an opaque pointer-events none overlay', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 300 });
  await page.setContent(
    '<body style="margin:0"><button id="under" style="position:absolute;top:20px;left:20px">search</button>' +
      '<div style="position:absolute;top:0;left:0;width:400px;height:100px;background:#000;pointer-events:none"></div></body>',
  );
  expect(await visibilityFailures(page, [{ name: 'under', selector: '#under' }])).toHaveLength(1);
});

test('self-test: the always-visible check fails on an element clipped away by its own clip-path', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 300 });
  await page.setContent('<body style="margin:0"><button id="self" style="position:absolute;top:20px;left:20px;clip-path:inset(50%)">search</button></body>');
  expect(await visibilityFailures(page, [{ name: 'self', selector: '#self' }])).toHaveLength(1);
});

test('self-test: the always-visible check passes an element under a transparent pointer-events none layer, with a rounded border and an icon drawn on it', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 300 });
  await page.setContent(
    '<body style="margin:0"><input id="pill" placeholder="Search" style="position:absolute;top:20px;left:20px;width:40px;height:30px;border-radius:20px;border:1px solid #888">' +
      '<svg style="position:absolute;top:25px;left:30px;pointer-events:none" width="20" height="20"><circle cx="10" cy="10" r="8" fill="#000"/></svg>' +
      '<div style="position:absolute;inset:0;pointer-events:none"></div></body>',
  );
  expect(await visibilityFailures(page, [{ name: 'pill', selector: '#pill' }])).toEqual([]);
});

// Gates 3 and 4: the screen-only user "arrives" only at a truth it can see.

test('self-test: a truth under the sticky header is not in view', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent(
    '<body style="margin:0"><nav class="navbar" style="position:fixed;top:0;left:0;right:0;height:60px;background:#fff;z-index:10">site</nav>' +
      '<main><p style="margin:10px 0 0">nothing planned runs today</p></main></body>',
  );
  expect([1, 2]).not.toContain(await page.evaluate(textGlance, { text: 'nothing planned runs today', navigation: NAVIGATION }));
});

test('self-test: a truth at opacity 0 is not in view', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent('<body style="margin:0"><main><p style="margin-top:200px;opacity:0">nothing planned runs today</p></main></body>');
  expect([1, 2]).not.toContain(await page.evaluate(textGlance, { text: 'nothing planned runs today', navigation: NAVIGATION }));
});

test('self-test: a truth in view below the sticky header is in view', async ({ page }) => {
  await page.setViewportSize({ width: MOBILE.width, height: MOBILE.height });
  await page.setContent(
    '<body style="margin:0"><nav class="navbar" style="position:fixed;top:0;left:0;right:0;height:60px;background:#fff;z-index:10">site</nav>' +
      '<main><p style="margin:100px 0 0">nothing planned runs today</p></main></body>',
  );
  expect(await page.evaluate(textGlance, { text: 'nothing planned runs today', navigation: NAVIGATION })).toBe(2);
});

// Gate 5.

test('self-test: the accessibility check fails on the serious-violation fixture', async ({ page }) => {
  await page.goto(`${ORIGIN}/__ux/fixtures/axe-serious.html`);
  const found = serious(await axe(page));
  expect(found.map((v) => `${v.impact} ${v.id}`).sort()).toEqual(['critical image-alt', 'serious color-contrast']);
});

// The ratchet.

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

function ratchet(name: string, measured: Measured, base: Baseline = baseline): { status: number | null; out: string } {
  const dir = join(OUT, 'self-test', name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'baseline.json'), JSON.stringify(base));
  writeFileSync(join(dir, 'measured.json'), JSON.stringify(measured));
  const run = spawnSync(process.execPath, [join(UX, 'ratchet.ts'), 'check', '--baseline', join(dir, 'baseline.json'), '--measured', join(dir, 'measured.json')], {
    encoding: 'utf8',
  });
  return { status: run.status, out: `${run.stdout}${run.stderr}` };
}

const withEntry = (entry: unknown): Baseline =>
  ({ ...baseline, scenarios: { ...baseline.scenarios, validate: { ...baseline.scenarios.validate, 'screen_only.actions': entry } } }) as Baseline;

const cases: { name: string; measured: Measured; base?: Baseline; status: number; says: string | null }[] = [
  { name: 'equal', measured: same, status: 0, says: null },
  { name: 'within-tolerance', measured: { ...same, validate: { ...same.validate, layout_shift: 0.025 } }, status: 0, says: null },
  { name: 'worse', measured: { ...same, validate: { ...same.validate, 'screen_only.actions': 4 } }, status: 1, says: 'worse validate screen_only.actions' },
  { name: 'worse-shift', measured: { ...same, validate: { ...same.validate, layout_shift: 0.04 } }, status: 1, says: 'worse validate layout_shift' },
  { name: 'improved', measured: { ...same, 'page.docs': { 'axe.moderate': 1, 'axe.minor': 0 } }, status: 1, says: 'improved-not-taken page.docs axe.moderate' },
  { name: 'missing-measured', measured: { ...same, validate: { 'screen_only.actions': 3, 'screen_only.looks': 4 } }, status: 1, says: 'missing validate layout_shift' },
  { name: 'missing-baseline', measured: { ...same, replay: { 'screen_only.actions': 2 } }, status: 1, says: 'missing replay screen_only.actions' },
  {
    name: 'loosened-tolerance',
    measured: { ...same, validate: { ...same.validate, 'screen_only.actions': 6 } },
    base: withEntry({ value: 3, tolerance: 5 }),
    status: 1,
    says: 'bad-baseline validate screen_only.actions',
  },
  { name: 'no-tolerance', measured: { ...same, validate: { ...same.validate, 'screen_only.actions': 30 } }, base: withEntry({ value: 3 }), status: 1, says: 'bad-baseline validate screen_only.actions' },
  { name: 'no-value', measured: same, base: withEntry({ value: null, tolerance: 0 }), status: 1, says: 'bad-baseline validate screen_only.actions' },
];

for (const c of cases) {
  test(`self-test: the ratchet exits ${c.status} on ${c.name}`, () => {
    const run = ratchet(c.name, c.measured, c.base);
    expect(run.status, run.out).toBe(c.status);
    if (c.says) expect(run.out).toContain(`ratchet: ${c.says}`);
    if (c.name === 'loosened-tolerance') expect(run.out).toContain('ratchet: worse validate screen_only.actions');
  });
}
