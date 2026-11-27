// What the harness does to one page: keeps it hermetic, sets its viewport and theme, waits until it
// has settled, and reads hard gates 1 (no horizontal scroll, nothing past its box), 2 (the
// always-visible elements fully in view) and 5 (axe-core) off it.

import { createRequire } from 'node:module';
import type { BrowserContext, Page } from '@playwright/test';
import { fullyVisible, overflow, signature } from './probe.ts';
import type { Required, Theme, Viewport } from './adapter.ts';

export interface AxeViolation {
  id: string;
  impact: string;
  help: string;
  nodes: string[];
}

/** Every request off the served site is answered by a stub page naming the URL: no request leaves the host. */
export async function hermetic(context: BrowserContext, origin: string): Promise<void> {
  await context.route('**/*', (route) => {
    const url = route.request().url();
    if (url.startsWith(origin) || url.startsWith('data:')) return route.continue();
    return route.fulfill({
      status: 200,
      contentType: 'text/html; charset=utf-8',
      body: `<!doctype html><html lang="en"><title>Off the site</title><main><h1>Off the site</h1><p>${url.replace(/</g, '&lt;')}</p></main></html>`,
    });
  });
}

export async function prepare(page: Page, viewport: Viewport, theme: Theme): Promise<void> {
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await page.emulateMedia({ colorScheme: theme.name, reducedMotion: 'reduce' });
}

/** Waits until the page is interactive and three reads of where it stands, 40 ms apart, agree (at most about 6 s). */
export async function settle(page: Page, notReady: string): Promise<void> {
  await page.waitForLoadState('load');
  await page.waitForFunction((sel) => !document.querySelector(sel), notReady, { timeout: 10_000 }).catch(() => undefined);
  // The reads run in the page, in one call: 40 ms apart, until three agree (at most 150).
  await inPage(page, `(async () => {
    const signature = ${signature.toString()};
    let last = '';
    let same = 0;
    for (let i = 0; i < 150 && same < 2; i += 1) {
      await new Promise((done) => setTimeout(done, 40));
      const now = signature();
      same = now === last ? same + 1 : 0;
      last = now;
    }
  })()`);
}

/**
 * After a resize or a scroll of a settled page: waits two animation frames at a time until two
 * reads of where the page stands agree (at most about 20 rounds). Cheaper than `settle`. The reads
 * run in the page, in one call.
 */
export async function quiet(page: Page): Promise<void> {
  await inPage(page, `(async () => {${QUIET_LOOP}})()`);
}

/** The loop of `quiet`, as in-page script: also the wait of `gatesHere` after it scrolls. */
const QUIET_LOOP = `
  let last = '';
  for (let i = 0; i < 20; i += 1) {
    const now = await new Promise((done) =>
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          const stops = document.querySelectorAll('[tabindex], a[href], button, input').length;
          done(location.href + '|' + Math.round(window.scrollY) + '|' + window.innerWidth + '|' + document.body.innerText.length + '|' + stops);
        }),
      ),
    );
    if (now === last) break;
    last = now;
  }`;

/**
 * Runs a wait in the page. A document that is replaced while it waits (a click that loads another
 * page) ends the call: the wait starts again in the new document once it has loaded, at most 3 times.
 */
async function inPage(page: Page, script: string): Promise<void> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      await page.evaluate(script);
      return;
    } catch (error) {
      if (attempt >= 3 || !/Execution context was destroyed|navigat/i.test(String(error))) throw error;
      await page.waitForLoadState('load');
    }
  }
}

/**
 * Switches a loaded page to the other theme in place: the site follows the system colour scheme
 * (`respectPrefersColorScheme`), so the emulated scheme changes and the page re-renders in that
 * theme as it does for a reader who switches the system theme. Waits until the page shows the
 * theme (at most 5 s, else it throws), then until three reads, two animation frames apart, of where
 * the page stands and of each drawn diagram's size and markup agree, as `settle` asks of a fresh
 * load (the diagrams are re-drawn in the new theme's colours).
 */
export async function switchTheme(page: Page, theme: Theme): Promise<void> {
  await page.emulateMedia({ colorScheme: theme.name, reducedMotion: 'reduce' });
  // In the page, in one call: the theme shown (each animation frame, at most 5 s), then the reads.
  const shown = await page.evaluate(async (name) => {
    const frame = () => new Promise<void>((done) => requestAnimationFrame(() => done()));
    const start = performance.now();
    while (document.documentElement.dataset.theme !== name && performance.now() - start < 5_000) await frame();
    if (document.documentElement.dataset.theme !== name) return document.documentElement.dataset.theme ?? '';
    let last = '';
    let same = 0;
    for (let i = 0; i < 40 && same < 2; i += 1) {
      await frame();
      await frame();
      const drawn = [...document.querySelectorAll('svg')].map((s) => {
        const r = s.getBoundingClientRect();
        return `${Math.round(r.width)}x${Math.round(r.height)}:${s.innerHTML.length}`;
      });
      const now = `${Math.round(window.scrollY)}|${document.documentElement.scrollHeight}|${document.body.innerText.length}|${drawn.join(',')}`;
      same = now === last ? same + 1 : 0;
      last = now;
    }
    return name;
  }, theme.name);
  if (shown !== theme.name) throw new Error(`the page shows theme "${shown}", not "${theme.name}"`);
}

/** The theme the page shows must be the one asked for: a theme check on the wrong theme proves nothing. */
export async function assertTheme(page: Page, theme: Theme): Promise<void> {
  const shown = await page.evaluate(() => document.documentElement.dataset.theme ?? '');
  if (shown !== theme.name) throw new Error(`the page shows theme "${shown}", not "${theme.name}"`);
}

/**
 * Every axe-core violation, each with its impact, of every rule axe-core runs by default (see
 * `AXE_OPTIONS`): gate 5 is not filtered by WCAG tag. Read in the page as `@axe-core/playwright`
 * does in legacy mode (the default mode opens a blank page per run to finish it, about 0.8 s each;
 * the site has no frames). axe-core is put into a document once and run there as often as asked:
 * the builder evaluated its 0.5 MB source twice per run.
 */
export async function axe(page: Page): Promise<AxeViolation[]> {
  const run = () =>
    page.evaluate(async (options) => {
      const core = (window as unknown as { axe?: { run: (context: Document, options: unknown) => Promise<AxeResults> } }).axe;
      if (!core || typeof core.run !== 'function') return null;
      const result = await core.run(document, options);
      return result.violations.map((v) => ({ id: v.id, impact: v.impact ?? 'unknown', help: v.help, nodes: v.nodes.map((n) => n.target.join(' ')) }));
    }, AXE_OPTIONS);
  const first = await run();
  if (first) return first;
  await page.evaluate(`${AXE_SOURCE}\n;axe.configure({ allowedOrigins: ['<same_origin>'], branding: { application: 'playwright' } });`);
  const result = await run();
  if (!result) throw new Error('axe-core did not load into the page');
  return result;
}

/** What one axe-core run reads back: its violations, each with the target of its nodes. */
interface AxeResults {
  violations: { id: string; impact?: string | null; help: string; nodes: { target: string[] }[] }[];
}

/**
 * The options of every run: no `runOnly`, so axe-core runs every rule it enables by default (the
 * WCAG A and AA rules and its best practices; the rules it ships disabled, such as the deprecated
 * ones, `color-contrast-enhanced` and `target-size`, stay off), and reports violations only. The
 * harness ran these rules before this file named them: its builder call `withTags([...])` was
 * followed by `options({ resultTypes })`, which replaced the tag filter.
 */
const AXE_OPTIONS = { resultTypes: ['violations'] };

/** The axe-core source `@axe-core/playwright` injects, resolved as that package resolves it. */
const AXE_SOURCE: string = (createRequire(require.resolve('@axe-core/playwright'))('axe-core') as { source: string }).source;

export const serious = (violations: readonly AxeViolation[]): AxeViolation[] => violations.filter((v) => v.impact === 'serious' || v.impact === 'critical');

export const nodesOf = (violations: readonly AxeViolation[], impact: string): number =>
  violations.filter((v) => v.impact === impact).reduce((n, v) => n + v.nodes.length, 0);

/** Hard gate 1 at the current scroll position, as one line per failure. */
export async function layoutFailures(page: Page): Promise<string[]> {
  return (await page.evaluate(overflow)).map((o) => `${o.kind} ${o.what} past by ${o.by}px`);
}

/** Hard gate 2 at the current scroll position, as one line per failure; an element marked `onlyWhere` counts only where its marker is. */
export async function visibilityFailures(page: Page, required: readonly Required[]): Promise<string[]> {
  const applicable = await page.evaluate(
    (list) => list.filter((r) => !r.onlyWhere || document.querySelector(r.onlyWhere)).map(({ name, selector }) => ({ name, selector })),
    [...required],
  );
  return (await page.evaluate(fullyVisible, applicable)).filter((v) => !v.ok).map((v) => `${v.name} ${v.why}`);
}

/**
 * Hard gates 1 and 2 in one call into the page, as `layoutFailures` and `visibilityFailures` read
 * them: first, when `scroll` names an end of the page, it scrolls there and waits as `quiet` does
 * (two animation frames at a time until two reads of where the page stands agree, at most 20
 * rounds). The probes are the same functions, sent to the page in one script.
 */
export async function gatesHere(page: Page, required: readonly Required[], scroll: 'top' | 'end' | null): Promise<{ layout: string[]; visibility: string[] }> {
  const read = await page.evaluate(`${READ_SOURCE}(${JSON.stringify({ scroll, required })})`);
  const { overflows, visible } = read as { overflows: { kind: string; what: string; by: number }[]; visible: { name: string; ok: boolean; why: string }[] };
  return {
    layout: overflows.map((o) => `${o.kind} ${o.what} past by ${o.by}px`),
    visibility: visible.filter((v) => !v.ok).map((v) => `${v.name} ${v.why}`),
  };
}

const READ_SOURCE = `(async ({ scroll, required }) => {
  const overflow = ${overflow.toString()};
  const fullyVisible = ${fullyVisible.toString()};
  if (scroll) {
    window.scrollTo(0, scroll === 'end' ? document.documentElement.scrollHeight : 0);
    ${QUIET_LOOP}
  }
  const applicable = required.filter((r) => !r.onlyWhere || document.querySelector(r.onlyWhere)).map(({ name, selector }) => ({ name, selector }));
  return { overflows: overflow(), visible: fullyVisible(applicable) };
})`;

/** Sums every layout shift without recent input from the first paint on, into `window.__uxShift`. Self-contained: sent to the page. */
export function shiftObserver(): void {
  const w = window as unknown as { __uxShift: number };
  w.__uxShift = 0;
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries() as unknown as { value: number; hadRecentInput: boolean }[]) if (!e.hadRecentInput) w.__uxShift += e.value;
    }).observe({ type: 'layout-shift', buffered: true });
  } catch {
    w.__uxShift = Number.NaN;
  }
}

export const shiftSoFar = (page: Page): Promise<number> => page.evaluate(() => (window as unknown as { __uxShift?: number }).__uxShift ?? Number.NaN);
