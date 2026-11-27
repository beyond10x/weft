// Adversary cases for weft-sim's search player (`ux/fixtures/sim-player.html`): what the fixture
// search does not reach. Its run has two axes of four short numbers, so the projection of a third
// axis, an axis of `vars` maps (each value a map of variables, `experiment.rs` `Mutation`), an axis
// of many values at the narrow viewport and the pickers' swap are never drawn by `sim-player.spec.ts`.
// Each case gives the page a snapshot of its own through `window.weftPlayer.receive`.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { GATING, ORIGIN, UX } from './adapter.ts';
import { hermetic } from './page.ts';

interface Axis {
  name: string;
  values: unknown[];
}

interface Row {
  seq: number;
  [key: string]: unknown;
}

interface Snapshot {
  info: { search: { axes: Axis[]; [key: string]: unknown }; [key: string]: unknown };
  search: Row[];
  [key: string]: unknown;
}

const URL = `${ORIGIN}/__ux/fixtures/sim-player.html`;
const SNAP = JSON.parse(/^let SNAP = (.*);$/m.exec(readFileSync(join(UX, 'fixtures', 'sim-player.html'), 'utf8'))![1]!) as Snapshot;
const ROWS = SNAP.search;
const AXES = SNAP.info.search.axes;
const LAST = ROWS[ROWS.length - 1]!;

const player = (page: Page) => page.locator('#player');
const button = (page: Page, name: string) => page.getByRole('button', { name, exact: true });

async function open(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.clock.install();
  await page.goto(URL);
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  return errors;
}

async function receive(page: Page, snap: unknown): Promise<void> {
  await page.evaluate((s) => (window as unknown as { weftPlayer: { receive(s: unknown): void } }).weftPlayer.receive(s), snap);
}

/** A finished snapshot of the fixture run with `axes` and `rows` in place of its own. */
function snapshot(axes: Axis[], rows: Row[]): Snapshot {
  return { ...SNAP, running: false, info: { ...SNAP.info, search: { ...SNAP.info.search, axes } }, search: rows };
}

/** One recorded row; `point` is built from the axes. */
function row(axes: Axis[], seq: number, decision: string, parent: number | null, index: number[], fitness: number, feasible: boolean, best: number | null): Row {
  const point = Object.fromEntries(axes.map((a, k) => [a.name, a.values[index[k]!]]));
  return { seq, round: seq, decision, parent, index, point, kpis: {}, fitness, feasible, best, walk: seq };
}

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

test('adversary: a third axis stays at the best point so far, and rows off that slice are faded', async ({ page }) => {
  const errors = await open(page);
  const axes: Axis[] = [
    { name: 'a', values: [1, 2] },
    { name: 'b', values: [10, 20] },
    { name: 'c', values: ['low', 'mid', 'high'] },
  ];
  // The best point (row 2) has c at index 1; the last row has c at index 2, the start c at index 0.
  const rows = [
    row(axes, 1, 'start', null, [0, 0, 0], 1, true, 1),
    row(axes, 2, 'accepted', 1, [0, 0, 1], 2, true, 2),
    row(axes, 3, 'rejected', 2, [1, 0, 1], 1.5, true, 2),
    row(axes, 4, 'rejected', 2, [1, 1, 2], 0.5, false, 2),
  ];
  await receive(page, snapshot(axes, rows));
  await expect(player(page)).toHaveAttribute('data-frame', '4');
  const cells = async () =>
    page.locator('[data-cell]').evaluateAll((all) => Object.fromEntries(all.map((e) => [e.getAttribute('data-cell')!, `${e.getAttribute('data-state')} ${e.getAttribute('data-fitness') ?? '-'}`])));
  expect(await cells(), 'at row 4 the slice is c = mid, the best point').toEqual({ '0,0': 'feasible 2', '1,0': 'feasible 1.5', '0,1': 'empty -', '1,1': 'empty -' });
  for (const [seq, off] of [
    ['1', true],
    ['2', false],
    ['3', false],
    ['4', true],
  ] as const) {
    const cls = (await page.locator(`[data-path-step="${seq}"]`).getAttribute('class')) ?? '';
    expect(cls.split(' ').includes('off'), `row ${seq} faded`).toBe(off);
  }
  await page.getByRole('slider', { name: 'Frame' }).fill('1');
  await expect(player(page)).toHaveAttribute('data-frame', '1');
  expect(await cells(), 'at row 1 the slice is c = low, the start').toEqual({ '0,0': 'feasible 1', '1,0': 'empty -', '0,1': 'empty -', '1,1': 'empty -' });
  expect(errors).toEqual([]);
});

test('adversary: choosing the other axis on one picker swaps the two axes', async ({ page }) => {
  const errors = await open(page);
  await page.getByRole('combobox', { name: 'Y axis' }).selectOption(AXES[0]!.name);
  await expect(player(page)).toHaveAttribute('data-y-axis', AXES[0]!.name);
  await expect(player(page), 'the x picker takes the axis y left').toHaveAttribute('data-x-axis', AXES[1]!.name);
  await page.getByRole('combobox', { name: 'X axis' }).selectOption(AXES[0]!.name);
  await expect(player(page)).toHaveAttribute('data-x-axis', AXES[0]!.name);
  await expect(player(page), 'the y picker takes the axis x left').toHaveAttribute('data-y-axis', AXES[1]!.name);
  await expect(page.locator('[data-cell]')).toHaveCount(AXES[0]!.values.length * AXES[1]!.values.length);
  expect(errors).toEqual([]);
});

test('adversary: step stops at either end, Play at the end plays again, a speed change applies mid-play, by keyboard', async ({ page }) => {
  const errors = await open(page);
  const first = ROWS[0]!.seq;
  // Play at the last row of a finished run.
  await button(page, 'Play').focus();
  await page.keyboard.press('Enter');
  await page.clock.runFor(300);
  await expect(player(page), 'Play at the last row of a finished run').toHaveAttribute('data-playing', 'true');
  expect(Number(await player(page).getAttribute('data-frame'))).toBeLessThan(LAST.seq);
  await button(page, 'Pause').click();

  // Step past either end.
  const slider = page.getByRole('slider', { name: 'Frame' });
  await slider.fill(String(first));
  await button(page, 'Step back').focus();
  await page.keyboard.press('Enter');
  await expect(player(page), 'Step back at the first row').toHaveAttribute('data-frame', String(first));
  await slider.fill(String(LAST.seq));
  await button(page, 'Step forward').focus();
  await page.keyboard.press('Space');
  await expect(player(page), 'Step forward at the last row').toHaveAttribute('data-frame', String(LAST.seq));
  await slider.focus();
  await page.keyboard.press('ArrowLeft');
  await expect(player(page), 'the slider by keyboard').toHaveAttribute('data-frame', String(LAST.seq - 1));

  // From 1 row a second to 16 while playing: the next row comes at the new speed, not after the old second.
  await slider.fill(String(first));
  await page.getByRole('combobox', { name: 'Speed' }).selectOption('1');
  await button(page, 'Play').click();
  await page.clock.runFor(200);
  await page.getByRole('combobox', { name: 'Speed' }).selectOption('16');
  await page.clock.runFor(600);
  expect(Number(await player(page).getAttribute('data-frame')), 'rows played in 600 ms at 16 rows per second').toBeGreaterThanOrEqual(first + 8);
  expect(errors).toEqual([]);
});

test('adversary: rows that arrive while playing at an earlier row leave the frame on that row', async ({ page }) => {
  const errors = await open(page);
  await receive(page, { ...SNAP, running: true });
  await page.getByRole('slider', { name: 'Frame' }).fill('5');
  await button(page, 'Play').click();
  await expect(player(page)).toHaveAttribute('data-playing', 'true');
  const last = ROWS[ROWS.length - 1]!;
  const more = [1, 2, 3].map((i) => ({ ...last, seq: last.seq + i, round: (last.round as number) + 1, decision: 'rejected', parent: last.seq, walk: last.seq + i }));
  await receive(page, { ...SNAP, running: true, search: [...ROWS, ...more] });
  await expect(player(page), 'the call itself, playing at row 5').toHaveAttribute('data-frame', '5');
  await page.clock.runFor(1000 / Number(await player(page).getAttribute('data-speed')) + 10);
  await expect(player(page), 'one row a tick from there').toHaveAttribute('data-frame', '6');
  expect(errors).toEqual([]);
});

// A `vars` axis: each value is a map of variables, drawn as its JSON text. Names and values carry
// markup, which must stay text; the tick labels must stay readable.
const VARS: Axis[] = [
  {
    name: 'pool <img src=x id=injected-x>',
    values: [
      { policy: 'round-robin', replicas: 1 },
      { policy: 'round-robin', replicas: 2 },
      { policy: 'least-loaded', replicas: 1 },
      { policy: 'least-loaded', replicas: 2 },
    ],
  },
  { name: 'zone"><img src=x id=injected-y>', values: ['eu-west-primary', 'eu-west-standby', '<b id=injected-v>us</b>'] },
];

test('adversary: a vars axis with markup in names and values: text only, tick labels apart and inside the landscape', async ({ page }) => {
  const errors = await open(page);
  await receive(page, snapshot(VARS, [row(VARS, 1, 'start', null, [0, 0], 1, true, 1), row(VARS, 2, 'accepted', 1, [1, 2], 2, true, 2)]));
  await expect(player(page)).toHaveAttribute('data-x-axis', VARS[0]!.name);
  await expect(page.locator('#injected-x, #injected-y, #injected-v')).toHaveCount(0);
  await expect(page.getByRole('combobox', { name: 'Y axis' }).locator('option')).toHaveText(VARS.map((a) => a.name));
  await page.getByRole('combobox', { name: 'X axis' }).selectOption(VARS[1]!.name);
  await expect(player(page)).toHaveAttribute('data-x-axis', VARS[1]!.name);
  await page.getByRole('combobox', { name: 'X axis' }).selectOption(VARS[0]!.name);
  await expect(player(page)).toHaveAttribute('data-x-axis', VARS[0]!.name);

  const land = (await page.locator('#p-land').boundingBox())!;
  const boxes = async (axis: string) =>
    page.locator(`[data-axis-label="${axis}"]`).evaluateAll((all) =>
      all.map((e) => {
        const r = e.getBoundingClientRect();
        return { index: Number(e.getAttribute('data-value-index')), left: r.left, right: r.right };
      }),
    );
  const outside = (await boxes('y')).filter((b) => b.left < land.x - 0.5).map((b) => `y ${b.index} starts ${Math.round(land.x - b.left)} px left of the landscape`);
  const x = (await boxes('x')).sort((a, b) => a.left - b.left);
  const overlapping = x.slice(1).flatMap((b, k) => (b.left < x[k]!.right - 0.5 ? [`x ${x[k]!.index} and ${b.index} overlap by ${Math.round(x[k]!.right - b.left)} px`] : []));
  expect([...outside, ...overlapping]).toEqual([]);
  expect(errors).toEqual([]);
});

test('adversary: the landscape of an axis of eight values is large enough to read at 390x844', async ({ page }) => {
  const mobile = GATING.find((v) => v.width === 390)!;
  await page.setViewportSize({ width: mobile.width, height: mobile.height });
  const errors = await open(page);
  const axes: Axis[] = [
    { name: 'replicas', values: [1, 2, 3, 4, 5, 6, 7, 8] },
    { name: 'closes', values: [0, 1] },
  ];
  await receive(page, snapshot(axes, [row(axes, 1, 'start', null, [0, 0], 1, true, 1), row(axes, 2, 'accepted', 1, [1, 0], 2, true, 2)]));
  await expect(page.locator('[data-cell]')).toHaveCount(16);
  // The bar of sim-player.spec.ts "landscape large enough to read", which only the fixture's 4 x 4 meets.
  const sizes = await page.locator('[data-cell]').evaluateAll((all) => all.map((e) => e.getBoundingClientRect()).map((r) => Math.min(r.width, r.height)));
  expect(Math.round(Math.min(...sizes)), 'the smallest cell, px').toBeGreaterThanOrEqual(44);
  const numbers = await page.locator('#p-land text.ct').evaluateAll((all) => all.map((e) => e.getBoundingClientRect().height));
  expect(Math.min(...numbers), 'the fitness numbers, px tall').toBeGreaterThanOrEqual(11);
  expect(errors).toEqual([]);
});
