// What the harness does to one page: keeps it hermetic, sets its viewport and theme, waits until it
// has settled, and reads hard gates 1 (no horizontal scroll, nothing past its box), 2 (the
// always-visible elements fully in view) and 5 (axe-core) off it.

import AxeBuilder from '@axe-core/playwright';
import type { BrowserContext, Page } from '@playwright/test';
import { fullyVisible, overflow, signature } from './probe.ts';
import type { Required, Theme, Viewport } from './adapter.ts';

export const AXE_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

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
  let last = '';
  let same = 0;
  for (let i = 0; i < 150 && same < 2; i += 1) {
    await page.waitForTimeout(40);
    const now = await page.evaluate(signature);
    same = now === last ? same + 1 : 0;
    last = now;
  }
}

/**
 * After a resize or a scroll of a settled page: waits two animation frames at a time until two
 * reads of where the page stands agree (at most about 20 rounds). Cheaper than `settle`.
 */
export async function quiet(page: Page): Promise<void> {
  let last = '';
  for (let i = 0; i < 20; i += 1) {
    const now = await page.evaluate(
      () =>
        new Promise<string>((done) =>
          requestAnimationFrame(() =>
            requestAnimationFrame(() => {
              const stops = document.querySelectorAll('[tabindex], a[href], button, input').length;
              done(`${location.href}|${Math.round(window.scrollY)}|${window.innerWidth}|${document.body.innerText.length}|${stops}`);
            }),
          ),
        ),
    );
    if (now === last) return;
    last = now;
  }
}

/** The theme the page shows must be the one asked for: a theme check on the wrong theme proves nothing. */
export async function assertTheme(page: Page, theme: Theme): Promise<void> {
  const shown = await page.evaluate(() => document.documentElement.dataset.theme ?? '');
  if (shown !== theme.name) throw new Error(`the page shows theme "${shown}", not "${theme.name}"`);
}

/**
 * Every axe-core violation of the gate's tags, each with its impact. Run in the page (legacy mode):
 * the default mode opens a blank page per run to finish it, about 0.8 s each; the site has no frames.
 */
export async function axe(page: Page): Promise<AxeViolation[]> {
  const result = await new AxeBuilder({ page }).withTags(AXE_TAGS).setLegacyMode(true).options({ resultTypes: ['violations'] }).analyze();
  return result.violations.map((v) => ({ id: v.id, impact: v.impact ?? 'unknown', help: v.help, nodes: v.nodes.map((n) => n.target.join(' ')) }));
}

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
