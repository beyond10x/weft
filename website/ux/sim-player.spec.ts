// The browser checks of weft-sim's search player: the page `ux/fixtures/sim-player.html`, which the
// Rust page test of weft-sim renders from its search fixture and holds byte for byte (`task
// sim-test`), opened with a fake clock (`page.clock`). The tests drive the controls by role and name
// and read the player's `data-*` state and the drawn elements by their data attributes. The expected
// numbers come from the snapshot inlined in the page, read here, never from the player. A script
// error on the page fails the test.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { GATING, ORIGIN, THEMES, UX } from './adapter.ts';
import { hermetic, layoutFailures, prepare, visibilityFailures } from './page.ts';

interface Axis {
  name: string;
  values: unknown[];
}

interface Row {
  seq: number;
  round: number;
  decision?: string;
  parent?: number | null;
  best?: number | null;
  [key: string]: unknown;
}

interface Snapshot {
  running?: boolean;
  info: { search: { axes: Axis[] } };
  run?: { spec?: string; out?: string; command?: string };
  search: Row[];
  [key: string]: unknown;
}

const FIXTURE = join(UX, 'fixtures', 'sim-player.html');
const URL = `${ORIGIN}/__ux/fixtures/sim-player.html`;

/** The snapshot inlined in the fixture page (`let SNAP = …;`, one line). */
function inlined(): Snapshot {
  const found = /^let SNAP = (.*);$/m.exec(readFileSync(FIXTURE, 'utf8'));
  if (!found) throw new Error(`${FIXTURE}: no inlined snapshot`);
  return JSON.parse(found[1]!) as Snapshot;
}

const SNAP = inlined();
const ROWS = SNAP.search;
const AXES = SNAP.info.search.axes;
const LAST = ROWS[ROWS.length - 1]!;
const valueText = (v: unknown): string => (typeof v === 'string' ? v : JSON.stringify(v));

/** Opens the fixture page with the fake clock installed; the script errors it raises, from load on. */
async function open(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.clock.install();
  await page.goto(URL);
  await expect(player(page)).not.toHaveAttribute('data-frame', '0');
  return errors;
}

const player = (page: Page) => page.locator('#player');
const button = (page: Page, name: string) => page.getByRole('button', { name, exact: true });
const frame = async (page: Page): Promise<number> => Number(await player(page).getAttribute('data-frame'));
const speed = async (page: Page): Promise<number> => Number(await player(page).getAttribute('data-speed'));

async function goTo(page: Page, k: number): Promise<void> {
  await page.getByRole('slider', { name: 'Frame' }).fill(String(k));
  await expect(player(page)).toHaveAttribute('data-frame', String(k));
}

/** `data-state` of every landscape cell, by its `data-cell`. */
async function cells(page: Page): Promise<Record<string, string>> {
  return page.locator('[data-cell]').evaluateAll((all) => Object.fromEntries(all.map((e) => [e.getAttribute('data-cell')!, e.getAttribute('data-state')!])));
}

async function receive(page: Page, snap: unknown): Promise<void> {
  await page.evaluate((s) => (window as unknown as { weftPlayer: { receive(s: unknown): void } }).weftPlayer.receive(s), snap);
}

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

test('the fixture run holds the cases the player checks', () => {
  expect(AXES.length).toBe(2);
  expect(ROWS.map((r) => r.seq)).toEqual(ROWS.map((_, i) => i + 1));
  for (const decision of ['start', 'accepted', 'rejected', 'restart']) expect(ROWS.some((r) => r.decision === decision), decision).toBe(true);
});

test('landscape cells', async ({ page }) => {
  const errors = await open(page);
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  await expect(player(page)).toHaveAttribute('data-x-axis', AXES[0]!.name);
  await expect(player(page)).toHaveAttribute('data-y-axis', AXES[1]!.name);
  await expect(page.locator('[data-cell]')).toHaveCount(AXES[0]!.values.length * AXES[1]!.values.length);
  expect(errors).toEqual([]);
});

test('axis labels', async ({ page }) => {
  const errors = await open(page);
  for (const [axis, at] of [
    ['x', 0],
    ['y', 1],
  ] as const) {
    const labels = await page
      .locator(`[data-axis-label="${axis}"]`)
      .evaluateAll((all) => all.map((e) => ({ index: Number(e.getAttribute('data-value-index')), text: e.textContent })));
    labels.sort((a, b) => a.index - b.index);
    expect(labels.map((l) => l.index)).toEqual(AXES[at]!.values.map((_, i) => i));
    expect(labels.map((l) => l.text)).toEqual(AXES[at]!.values.map(valueText));
  }
  expect(errors).toEqual([]);
});

test('step adds one row', async ({ page }) => {
  const errors = await open(page);
  const k = Math.floor(ROWS.length / 2);
  await goTo(page, k);
  const steps = page.locator('[data-path-step]');
  const points = page.locator('[data-curve-point]');
  const before = { steps: await steps.count(), points: await points.count(), cells: await cells(page) };
  expect(before.steps).toBeGreaterThan(0);

  await button(page, 'Step forward').click();
  await expect(player(page)).toHaveAttribute('data-frame', String(k + 1));
  await expect(steps).toHaveCount(before.steps + 1);
  await expect(page.locator(`[data-path-step="${k + 1}"]`)).toHaveCount(1);
  await expect(points).toHaveCount(before.points + 1);
  const after = await cells(page);
  expect(Object.keys(after).sort()).toEqual(Object.keys(before.cells).sort());
  expect(Object.keys(after).filter((c) => after[c] !== before.cells[c]).length).toBeLessThanOrEqual(1);

  await button(page, 'Step back').click();
  await expect(player(page)).toHaveAttribute('data-frame', String(k));
  await expect(steps).toHaveCount(before.steps);
  await expect(points).toHaveCount(before.points);
  expect(await cells(page)).toEqual(before.cells);
  expect(errors).toEqual([]);
});

test('decision marks', async ({ page }) => {
  const errors = await open(page);
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  for (const decision of ['accepted', 'rejected', 'restart']) {
    const rows = ROWS.filter((r) => r.decision === decision).length;
    await expect(page.locator(`[data-path-step][data-decision="${decision}"]`), decision).toHaveCount(rows);
  }
  for (const row of ROWS.filter((r) => r.decision === 'accepted')) {
    await expect(page.locator(`[data-path-step="${row.seq}"]`)).toHaveAttribute('data-parent', String(row.parent));
  }
  expect(errors).toEqual([]);
});

test('curve ends at the best', async ({ page }) => {
  const errors = await open(page);
  const last = page.locator('[data-curve-point]').last();
  await expect(last).toHaveAttribute('data-curve-point', String(LAST.seq));
  await expect(last).toHaveAttribute('data-best', String(LAST.best));
  expect(errors).toEqual([]);
});

test('play runs to the last row and stops', async ({ page }) => {
  const errors = await open(page);
  await goTo(page, 1);
  await button(page, 'Play').click();
  await expect(player(page)).toHaveAttribute('data-playing', 'true');
  await page.clock.runFor(((LAST.seq - 1) * 1000) / (await speed(page)) + 1000);
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  await expect(player(page)).toHaveAttribute('data-playing', 'false');
  expect(errors).toEqual([]);
});

test('pause holds the frame', async ({ page }) => {
  const errors = await open(page);
  await goTo(page, 1);
  await button(page, 'Play').click();
  await page.clock.runFor(1000);
  await button(page, 'Pause').click();
  const held = await frame(page);
  expect(held).toBeGreaterThan(1);
  await page.clock.runFor(5000);
  await expect(player(page)).toHaveAttribute('data-frame', String(held));
  await expect(player(page)).toHaveAttribute('data-playing', 'false');
  expect(errors).toEqual([]);
});

test('speed sets the rows per second', async ({ page }) => {
  const errors = await open(page);
  for (const rows of [2, 4]) {
    await goTo(page, 1);
    await page.getByRole('combobox', { name: 'Speed' }).selectOption(String(rows));
    await expect(player(page)).toHaveAttribute('data-speed', String(rows));
    await button(page, 'Play').click();
    await page.clock.runFor(1000);
    await expect(player(page)).toHaveAttribute('data-frame', String(1 + rows));
    await button(page, 'Pause').click();
  }
  expect(errors).toEqual([]);
});

test('axis picker redraws', async ({ page }) => {
  const errors = await open(page);
  const k = Math.floor(ROWS.length / 2);
  await goTo(page, k);
  const before = await cells(page);
  await page.getByRole('combobox', { name: 'X axis' }).selectOption(AXES[1]!.name);
  await page.getByRole('combobox', { name: 'Y axis' }).selectOption(AXES[0]!.name);
  await expect(player(page)).toHaveAttribute('data-x-axis', AXES[1]!.name);
  await expect(player(page)).toHaveAttribute('data-y-axis', AXES[0]!.name);
  await expect(player(page)).toHaveAttribute('data-frame', String(k));
  const after = await cells(page);
  expect(Object.keys(after).length).toBe(Object.keys(before).length);
  for (const [cell, state] of Object.entries(before)) {
    const [i, j] = cell.split(',');
    expect(after[`${j},${i}`], `cell ${j},${i} after, ${cell} before`).toBe(state);
  }
  expect(errors).toEqual([]);
});

/** `n` rows after the last of `rows`: rejected neighbours of the last row, in a round of their own. */
function more(rows: Row[], n: number): Row[] {
  const last = rows[rows.length - 1]!;
  return Array.from({ length: n }, (_, i) => ({ ...last, seq: last.seq + 1 + i, round: last.round + 1, decision: 'rejected', parent: last.seq, walk: last.seq + 1 + i }));
}

test('live rows extend the timeline and the player follows them only from the last row', async ({ page }) => {
  const errors = await open(page);
  // A live run goes on: the player playing at its last row waits for rows.
  await receive(page, { ...SNAP, running: true });
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  await button(page, 'Play').click();
  await expect(player(page)).toHaveAttribute('data-playing', 'true');
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  const three = [...ROWS, ...more(ROWS, 3)];
  await receive(page, { ...SNAP, running: true, search: three });
  await page.clock.runFor((3 * 1000) / (await speed(page)) + 500);
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq + 3));
  await expect(player(page)).toHaveAttribute('data-playing', 'true');

  // At an earlier row the same call leaves the frame on that row.
  await button(page, 'Pause').click();
  await goTo(page, 5);
  await receive(page, { ...SNAP, running: true, search: [...three, ...more(three, 3)] });
  await page.clock.runFor(2000);
  await expect(player(page)).toHaveAttribute('data-frame', '5');
  await expect(page.locator('[data-curve-point]')).toHaveCount(5);
  expect(errors).toEqual([]);
});

test('result without a climb', async ({ page }) => {
  const errors = await open(page);
  const old = ROWS.map((row) => {
    const { decision: _d, parent: _p, best: _b, walk: _w, ...rest } = row;
    return rest;
  });
  await receive(page, { ...SNAP, search: old });
  await expect(page.locator('#search tr:has(td)')).toHaveCount(old.length);
  await expect(page.locator('[data-no-climb]')).toHaveCount(1);
  await expect(page.locator('[data-path-step]')).toHaveCount(0);
  await page.clock.runFor(2000);
  expect(errors).toEqual([]);
});

// `fixtures/sim-player-markup.html`: the same run with `<!--<script>` as the fourth value of the
// first axis and `</script><!--<script>` in a KPI of every row, inlined by weft-sim (its Rust test
// `the_player_page_inlines_markup_in_recorded_values_as_data` holds the page). No `-->` follows,
// which would end the parser state the texts open. The page must still parse, run its script and
// show both texts as text.
test('markup in recorded values stays text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.clock.install();
  await page.goto(`${ORIGIN}/__ux/fixtures/sim-player-markup.html`);
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq));
  await expect(page.locator('script')).toHaveCount(1);
  await expect(page.locator('[data-axis-label="x"][data-value-index="3"]')).toHaveText('<!--<script>');
  await expect(page.locator('#search td', { hasText: '</script><!--<script>' })).toHaveCount(ROWS.length);
  await expect(page.locator('[data-path-step]')).toHaveCount(ROWS.length);
  expect(errors).toEqual([]);
});

// The page as a reader sees it: every control in view, the axes in value order, the points table
// in its own scroll box with rows of one height, and a landscape large enough to read.

const CONTROLS = ['p-back', 'p-play', 'p-pause', 'p-fwd', 'p-frame', 'p-speed', 'p-x', 'p-y'].map((id) => ({ name: id, selector: `#${id}` }));

for (const viewport of GATING) {
  for (const theme of THEMES) {
    test(`controls in view below the header at ${viewport.name} ${theme.name}`, async ({ page }) => {
      await prepare(page, viewport, theme);
      const errors = await open(page);
      // The section and the bar, each scrolled into view as a reader's jump or a screenshot does.
      for (const target of ['#search-box', '#player .controls']) {
        await page.locator(target).scrollIntoViewIfNeeded();
        expect(await visibilityFailures(page, CONTROLS), `after scrolling ${target} into view`).toEqual([]);
      }
      expect(errors).toEqual([]);
    });
  }
}

/** The order an axis's values are drawn in, lowest first: by value when all are numbers, else as declared. */
function drawnOrder(values: unknown[]): number[] {
  const order = values.map((_, i) => i);
  if (values.every((v) => typeof v === 'number')) order.sort((a, b) => (values[a] as number) - (values[b] as number));
  return order;
}

test('axis ticks in value order, lowest at the origin', async ({ page }) => {
  const errors = await open(page);
  expect(drawnOrder(AXES[1]!.values), 'the fixture declares an axis out of value order').not.toEqual(AXES[1]!.values.map((_, i) => i));
  for (const [axis, at] of [
    ['x', 0],
    ['y', 1],
  ] as const) {
    const ticks = await page.locator(`[data-axis-label="${axis}"]`).evaluateAll((all) =>
      all.map((e) => {
        const r = e.getBoundingClientRect();
        return { index: Number(e.getAttribute('data-value-index')), x: r.left + r.width / 2, y: r.top + r.height / 2 };
      }),
    );
    // Left to right on x, bottom to top on y.
    ticks.sort((a, b) => (axis === 'x' ? a.x - b.x : b.y - a.y));
    expect(ticks.map((t) => t.index), `${axis} axis`).toEqual(drawnOrder(AXES[at]!.values));
  }
  // Each cell sits over its x tick and beside its y tick.
  const cells = await page.locator('[data-cell]').evaluateAll((all) =>
    all.map((e) => {
      const r = e.getBoundingClientRect();
      const [i, j] = e.getAttribute('data-cell')!.split(',').map(Number);
      return { i: i!, j: j!, x: r.left + r.width / 2, y: r.top + r.height / 2 };
    }),
  );
  for (const c of cells) {
    const tx = (await page.locator(`[data-axis-label="x"][data-value-index="${c.i}"]`).boundingBox())!;
    const ty = (await page.locator(`[data-axis-label="y"][data-value-index="${c.j}"]`).boundingBox())!;
    expect(Math.abs(tx.x + tx.width / 2 - c.x), `cell ${c.i},${c.j} over its x tick`).toBeLessThan(2);
    expect(Math.abs(ty.y + ty.height / 2 - c.y), `cell ${c.i},${c.j} beside its y tick`).toBeLessThan(4);
  }
  expect(errors).toEqual([]);
});

for (const viewport of GATING) {
  test(`points table rows of one height in a box of its own, no page overflow at ${viewport.name}`, async ({ page }) => {
    await prepare(page, viewport, THEMES[0]!);
    const errors = await open(page);
    const heights = await page.locator('#search tr:has(td)').evaluateAll((all) => all.map((e) => e.getBoundingClientRect().height));
    expect(heights.length).toBe(ROWS.length);
    expect(Math.max(...heights) / Math.min(...heights), `row heights ${heights.map(Math.round).join(' ')}`).toBeLessThanOrEqual(1.1);
    const box = await page.locator('#search table').evaluate((t) => {
      const s = getComputedStyle(t.parentElement!);
      return { overflowX: s.overflowX, wider: t.getBoundingClientRect().width > t.parentElement!.getBoundingClientRect().width + 1 };
    });
    expect(['auto', 'scroll'], `the table's box scrolls (the table is ${box.wider ? 'wider' : 'not wider'} than it)`).toContain(box.overflowX);
    for (const at of ['top', '#player', '#search']) {
      if (at !== 'top') await page.locator(at).scrollIntoViewIfNeeded();
      expect(await layoutFailures(page), `gate 1 at ${at}`).toEqual([]);
    }
    expect(errors).toEqual([]);
  });

  test(`landscape large enough to read at ${viewport.name}`, async ({ page }) => {
    await prepare(page, viewport, THEMES[0]!);
    const errors = await open(page);
    const sizes = await page.locator('[data-cell]').evaluateAll((all) => all.map((e) => e.getBoundingClientRect()).map((r) => Math.min(r.width, r.height)));
    expect(Math.min(...sizes), 'the smallest cell, px').toBeGreaterThanOrEqual(44);
    const numbers = await page.locator('#p-land text.ct').evaluateAll((all) => all.map((e) => e.getBoundingClientRect().height));
    expect(numbers.length).toBeGreaterThan(0);
    expect(Math.min(...numbers), 'the fitness numbers, px tall').toBeGreaterThanOrEqual(11);
    const marks = await page.locator('[data-path-step]').evaluateAll((all) => all.map((e) => e.getBoundingClientRect()).map((r) => Math.max(r.width, r.height)));
    expect(Math.min(...marks), 'the smallest path mark, px').toBeGreaterThanOrEqual(10);
    const land = (await page.locator('#p-land').boundingBox())!;
    expect(land.x + land.width, 'the landscape inside the viewport').toBeLessThanOrEqual(viewport.width);
    expect(errors).toEqual([]);
  });
}

test('a landscape wider than the screen scrolls in its own box at 390x844, the page does not', async ({ page }) => {
  await prepare(page, GATING.find((v) => v.width === 390)!, THEMES[0]!);
  const errors = await open(page);
  const axes = [
    { name: 'replicas', values: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] },
    { name: 'closes', values: [0, 1] },
  ];
  const rows = [
    { seq: 1, round: 1, decision: 'start', parent: null, index: [0, 0], point: { replicas: 1, closes: 0 }, kpis: {}, fitness: 1, feasible: true, best: 1, walk: 1 },
    { seq: 2, round: 2, decision: 'accepted', parent: 1, index: [9, 0], point: { replicas: 10, closes: 0 }, kpis: {}, fitness: 2, feasible: true, best: 2, walk: 2 },
  ];
  await receive(page, { ...SNAP, info: { ...SNAP.info, search: { ...SNAP.info.search, axes } }, search: rows });
  await expect(page.locator('[data-cell]')).toHaveCount(20);
  const sizes = await page.locator('[data-cell]').evaluateAll((all) => all.map((e) => e.getBoundingClientRect()).map((r) => Math.min(r.width, r.height)));
  expect(Math.round(Math.min(...sizes)), 'the smallest cell, px').toBeGreaterThanOrEqual(44);
  const box = await page.locator('#p-land').evaluate((svg) => ({ scroll: svg.parentElement!.scrollWidth, client: svg.parentElement!.clientWidth }));
  expect(box.scroll, 'the landscape is wider than its box and scrolls in it').toBeGreaterThan(box.client);
  await page.locator('#player').scrollIntoViewIfNeeded();
  expect(await layoutFailures(page)).toEqual([]);
  await page.locator('[data-cell="9,0"]').scrollIntoViewIfNeeded();
  expect(await visibilityFailures(page, [{ name: 'the last column', selector: '[data-cell="9,0"]' }])).toEqual([]);
  expect(errors).toEqual([]);
});

// The replay of the selected point: its walk number and the command that reruns it.

const selectedWalk = (page: Page) => page.locator('[data-selected-walk]');
const replayCommand = (page: Page) => page.locator('[data-replay-command]');

test('selected point shows its replay', async ({ page }) => {
  const errors = await open(page);
  const row = ROWS.find((r) => r.decision === 'rejected')!;
  await page.locator(`[data-path-step="${row.seq}"]`).click();
  await expect(selectedWalk(page)).toHaveText(String(row.walk));
  await expect(replayCommand(page)).toHaveText(/^weft-sim replay /);
  const words = (await replayCommand(page).textContent())!.split(/\s+/);
  const after = (flag: string) => words[words.indexOf(flag) + 1];
  expect(after('--walk')).toBe(String(row.walk));
  expect(after('--spec')).toBe(SNAP.run!.spec);
  expect(after('--trace')).toBe(`${SNAP.run!.out}/result.json`);
  expect(words.filter((w) => w.startsWith('/') || w.startsWith('~')), 'no absolute path').toEqual([]);
  expect(errors).toEqual([]);
});

test('the selection is the frame row until a cell or a mark is clicked, by walk not seq, and follows the keyboard', async ({ page }) => {
  const errors = await open(page);
  // Walks unlike seqs, as in a run whose rounds ran several points at once.
  const rows: Row[] = ROWS.map((r) => ({ ...r, walk: 1000 + r.seq * 7 }));
  await receive(page, { ...SNAP, search: rows });
  const at = (seq: number) => String(rows[seq - 1]!.walk);
  await expect(selectedWalk(page), 'the row of the last frame').toHaveText(at(LAST.seq));
  await button(page, 'Step back').click();
  await expect(selectedWalk(page), 'the row of the frame stepped to').toHaveText(at(LAST.seq - 1));

  // A cell, clicked off its mark: the row drawn in it.
  const cellRow = rows.find((r) => r.decision === 'accepted' && r.seq < LAST.seq - 2)!;
  const [i, j] = cellRow.index as number[];
  await page.locator(`[data-cell="${i},${j}"]`).click({ position: { x: 5, y: 5 } });
  await expect(selectedWalk(page)).toHaveText(String(cellRow.walk));
  await expect(replayCommand(page)).toContainText(`--walk ${cellRow.walk}`);
  // A mark.
  const markRow = rows.find((r) => r.decision === 'restart')!;
  if (markRow.seq <= LAST.seq - 1) {
    await page.locator(`[data-path-step="${markRow.seq}"]`).click();
    await expect(selectedWalk(page)).toHaveText(String(markRow.walk));
  }
  // The keyboard moves the frame, and the selection with it.
  await page.getByRole('slider', { name: 'Frame' }).focus();
  await page.keyboard.press('ArrowLeft');
  await expect(player(page)).toHaveAttribute('data-frame', String(LAST.seq - 2));
  await expect(selectedWalk(page)).toHaveText(at(LAST.seq - 2));
  expect(errors).toEqual([]);
});

test('a replay command of odd paths stays one quoted text inside the screen at 390x844', async ({ page }) => {
  await prepare(page, GATING.find((v) => v.width === 390)!, THEMES[0]!);
  const errors = await open(page);
  const out = `runs/it's here/<b id=injected>x</b>/${'a-long-directory-name/'.repeat(6)}out`;
  await receive(page, { ...SNAP, run: { spec: 'ess', out } });
  await expect(page.locator('#injected')).toHaveCount(0);
  const quoted = `'${`${out}/result.json`.replace(/'/g, `'\\''`)}'`;
  await expect(replayCommand(page)).toContainText(`--spec ess --trace ${quoted} --walk ${LAST.walk}`);
  await replayCommand(page).scrollIntoViewIfNeeded();
  expect(await layoutFailures(page)).toEqual([]);
  expect(await visibilityFailures(page, [{ name: 'replay command', selector: '[data-replay-command]' }])).toEqual([]);
  // A result written before the run named its paths: they come from its command line.
  await receive(page, { ...SNAP, run: { command: 'weft-sim simulate --spec specs/a --experiment e.yaml --out sim/out/b --threads 2' } });
  await expect(replayCommand(page)).toContainText('--spec specs/a --trace sim/out/b/result.json');
  expect(errors).toEqual([]);
});

// Adversary: the clickable marks from the keyboard, and a word a shell expands.

test('adversary: a landscape mark is focusable, named, and selected with Enter', async ({ page }) => {
  const errors = await open(page);
  const row = ROWS.find((r) => r.decision === 'rejected' && r.seq !== LAST.seq)!;
  const mark = page.locator(`[data-path-step="${row.seq}"]`);
  const focusable = await page
    .locator('#p-land [data-path-step], #p-land [data-cell][data-seq]')
    .evaluateAll((all) => all.filter((e) => (e as SVGElement).tabIndex >= 0).length);
  expect(focusable, 'marks and cells with a row are in the Tab order').toBeGreaterThan(0);
  await mark.focus();
  await expect(mark).toBeFocused();
  await expect(mark).toHaveAccessibleName(/\S/);
  await page.keyboard.press('Enter');
  await expect(selectedWalk(page)).toHaveText(String(row.walk));
  expect(errors).toEqual([]);
});

test('adversary: a path word starting with = is quoted (zsh EQUALS expands an unquoted =word)', async ({ page }) => {
  const errors = await open(page);
  await receive(page, { ...SNAP, run: { spec: '=specs', out: 'sim/out/x' } });
  await expect(replayCommand(page)).toContainText("--spec '=specs' ");
  expect(errors).toEqual([]);
});

test('every landscape mark and cell with a row is a named button; Space selects a cell and its focus stays shown', async ({ page }) => {
  const errors = await open(page);
  // An early frame, so that some cells have no row yet.
  await goTo(page, 3);
  const drawn = await page.locator('#p-land [data-path-step], #p-land [data-cell]').evaluateAll((all) =>
    all.map((e) => ({
      what: e.getAttribute('data-path-step') ?? e.getAttribute('data-cell'),
      row: e.hasAttribute('data-path-step') || e.hasAttribute('data-seq'),
      tab: (e as SVGElement).tabIndex,
      role: e.getAttribute('role'),
      label: e.getAttribute('aria-label') ?? '',
    })),
  );
  expect(drawn.some((d) => !d.row), 'the fixture draws empty cells').toBe(true);
  for (const d of drawn) {
    if (d.row) expect([d.tab, d.role, d.label !== ''], `${d.what}: in the Tab order, a named button`).toEqual([0, 'button', true]);
    else expect([d.tab, d.role], `${d.what}: no row, not focusable`).toEqual([-1, null]);
  }

  const cell = page.locator('#p-land [data-cell][data-seq]').first();
  const where = (await cell.getAttribute('data-cell'))!;
  const seq = Number(await cell.getAttribute('data-seq'));
  const row = ROWS.find((r) => r.seq === seq)!;
  const point = row.point as Record<string, unknown>;
  for (const axis of AXES) await expect(cell, axis.name).toHaveAccessibleName(new RegExp(`${axis.name} ${valueText(point[axis.name])}\\b`));
  await expect(cell).toHaveAccessibleName(new RegExp(`walk ${row.walk}\\b`));
  await cell.focus();
  await page.keyboard.press(' ');
  await expect(selectedWalk(page)).toHaveText(String(row.walk));
  const again = page.locator(`#p-land [data-cell="${where}"]`);
  await expect(again, 'the focus stays on the cell after the redraw').toBeFocused();
  expect(await again.evaluate((e) => getComputedStyle(e).outlineStyle), 'the focus is shown').not.toBe('none');
  expect(errors).toEqual([]);
});

test('a path word starting with ~ or % is quoted, an = inside a word is not', async ({ page }) => {
  const errors = await open(page);
  await receive(page, { ...SNAP, run: { spec: '~/specs', out: '%1' } });
  await expect(replayCommand(page)).toContainText("--spec '~/specs' --trace '%1/result.json' ");
  await receive(page, { ...SNAP, run: { spec: 'a=b', out: 'sim/out/x' } });
  await expect(replayCommand(page)).toContainText('--spec a=b --trace sim/out/x/result.json ');
  expect(errors).toEqual([]);
});
