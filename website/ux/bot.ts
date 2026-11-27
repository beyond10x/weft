// The screen-only user (ui-iteration-loop § 7, docs variant): deterministic code, no model. It uses
// only controls fully inside the viewport and not covered at their centre, found by role and name as
// a sighted reader reads them; a click, a key press or a screen of scrolling is one action.
//
// From the scenario's start page it follows the usable control whose name (or section heading)
// carries the best of the scenario's words; after a link that carried the first word it reads on to
// the end of what it led to. With no match it opens a control that reveals hidden navigation (once
// per page), closes revealed navigation that holds no match, types the first word into the search
// field (once per play) and picks a result, and else scrolls one screen. It is done when the truth's
// text is inside the viewport on the truth's page.
//
// Each action notes a look: 1 when the control was in a navigation region, 2 elsewhere in view, 3
// for a screen of scrolling. `looks` is their sum.

import type { Page } from '@playwright/test';
import { BASE, DISMISS, NAVIGATION, NOT_READY, REVEAL, SEARCH } from './adapter.ts';
import type { Control, Scenario, Theme, Viewport } from './adapter.ts';
import { assertTheme, prepare, quiet, settle, shiftObserver, shiftSoFar } from './page.ts';
import { candidates, textGlance } from './probe.ts';
import type { Candidate } from './probe.ts';

/** Most actions one scenario may take. */
export const MAX_ACTIONS = 30;

export interface Play {
  scenario: string;
  viewport: string;
  theme: string;
  reached: boolean;
  actions: number;
  looks: number;
  layout_shift: number;
  path: string[];
  stuck: string | null;
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** `word` occurs in `text` at the start of a word, ignoring case. */
export const carries = (text: string, word: string): boolean => new RegExp(`(^|[^\\p{L}\\p{N}])${escape(word)}`, 'iu').test(text);

/** The rank of the best word a control's name or its section heading carries; -1 for none. */
export function rank(c: Pick<Candidate, 'name' | 'context'>, words: readonly string[]): number {
  for (let i = 0; i < words.length; i += 1) if (carries(c.name, words[i]!)) return i * 2;
  for (let i = 0; i < words.length; i += 1) if (carries(c.context, words[i]!)) return i * 2 + 1;
  return -1;
}

const usable = (c: Candidate): boolean => c.inView && !c.covered;

/** What a control leads to: its target for a link, else its role and name. */
const target = (c: Pick<Candidate, 'href' | 'role' | 'name'>): string => c.href ?? `${c.role}|${c.name}`;

const lookOf = (c: Pick<Candidate, 'navigation'>): number => (c.navigation ? 1 : 2);

const isSearchField = (c: Pick<Candidate, 'role' | 'name'>, search: Control): boolean => c.name === search.name && (c.role === search.role || c.role === 'combobox');

/** A path below the base without `.html` or a trailing slash, for comparing pages. */
const normal = (path: string): string => path.replace(/\.html$/, '').replace(/\/+$/, '');

/** The next control: the best word match among usable controls not used before and not leading here. */
function choose(all: readonly Candidate[], words: readonly string[], used: ReadonlySet<string>, here: string): Candidate | null {
  const ranked = all
    .filter((c) => usable(c) && !used.has(target(c)) && c.href !== here)
    .map((c) => ({ c, r: rank(c, words) }))
    .filter((x) => x.r >= 0);
  ranked.sort((a, b) => a.r - b.r);
  return ranked[0]?.c ?? null;
}

/** While reading: a usable link in the text (not navigation) whose name carries a word. */
function readingPick(all: readonly Candidate[], words: readonly string[], used: ReadonlySet<string>, here: string): Candidate | null {
  return choose(
    all.filter((c) => c.role === 'link' && !c.navigation && rank(c, words) % 2 === 0),
    words,
    used,
    here,
  );
}

export async function play(page: Page, scenario: Scenario, viewport: Viewport, theme: Theme): Promise<Play> {
  await prepare(page, viewport, theme);
  await page.addInitScript(shiftObserver);
  const record: Play = { scenario: scenario.id, viewport: viewport.name, theme: theme.name, reached: false, actions: 0, looks: 0, layout_shift: 0, path: [], stuck: null };
  const basePath = new URL(BASE).pathname;
  const truthPath = normal(`${basePath}${scenario.truth.path}`);
  const used = new Set<string>();
  let searched = false;
  let reading = false;
  const onSite = () => page.url().startsWith(BASE);
  const pathNow = () => normal(new URL(page.url()).pathname);
  const arrived = async (): Promise<boolean> => {
    if (!onSite() || pathNow() !== truthPath) return false;
    const g = await page.evaluate(textGlance, { text: scenario.truth.text, navigation: NAVIGATION });
    return g === 1 || g === 2;
  };
  const note = (look: number, what: string) => {
    record.actions += 1;
    record.looks += look;
    record.path.push(what);
  };
  const click = async (pick: Candidate, what: string) => {
    await page.mouse.click(pick.box.x + pick.box.width / 2, pick.box.y + pick.box.height / 2);
    // A finger leaves no hover behind.
    await page.mouse.move(0, 0);
    note(lookOf(pick), `${pick.role} "${pick.name}"${what}`);
    await settle(page, NOT_READY);
  };
  const scroll = async (what: string): Promise<boolean> => {
    const y = await page.evaluate(() => window.scrollY);
    await page.evaluate(() => window.scrollBy(0, Math.round(window.innerHeight * 0.85)));
    await quiet(page);
    if ((await page.evaluate(() => window.scrollY)) <= y) return false;
    note(3, what);
    return true;
  };
  try {
    await page.goto(`${BASE}${scenario.start}`);
    await settle(page, NOT_READY);
    await assertTheme(page, theme);
    while (record.actions < MAX_ACTIONS) {
      if (await arrived()) {
        record.reached = true;
        break;
      }
      if (!onSite()) {
        note(2, 'back (off the site)');
        await page.goBack();
        await settle(page, NOT_READY);
        continue;
      }
      if (reading) {
        const seen = readingPick(await page.evaluate(candidates, NAVIGATION), scenario.words, used, page.url());
        if (seen) {
          used.add(target(seen));
          const from = page.url();
          await click(seen, '');
          reading = page.url() !== from && onSite();
          continue;
        }
        if (await scroll('scroll (reading)')) continue;
        reading = false;
      }
      const here = page.url();
      const all = await page.evaluate(candidates, NAVIGATION);
      const pick = choose(all, scenario.words, used, here);
      if (pick) {
        used.add(target(pick));
        await click(pick, '');
        reading = rank(pick, scenario.words) === 0 && page.url() !== here && onSite();
        continue;
      }
      const reveal = REVEAL.map((r) => all.find((c) => usable(c) && c.role === r.role && c.name === r.name)).find((c) => c && !used.has(`${pathNow()}|${c.role}|${c.name}`));
      if (reveal) {
        used.add(`${pathNow()}|${reveal.role}|${reveal.name}`);
        await click(reveal, ' (reveal)');
        continue;
      }
      const close = all.find((c) => usable(c) && DISMISS.some((d) => d.role === c.role && d.name === c.name));
      if (close) {
        await click(close, ' (dismiss)');
        continue;
      }
      const field = all.find((c) => usable(c) && isSearchField(c, SEARCH));
      if (field && !searched) {
        searched = true;
        await page.mouse.click(field.box.x + field.box.width / 2, field.box.y + field.box.height / 2);
        note(lookOf(field), `${field.role} "${field.name}" (search)`);
        await page.keyboard.type(scenario.words[0]!);
        note(1, `type "${scenario.words[0]}"`);
        await page.waitForSelector('[role=option]', { timeout: 3000 }).catch(() => undefined);
        await settle(page, NOT_READY);
        const results = (await page.evaluate(candidates, NAVIGATION)).filter((c) => c.role === 'option');
        if (!choose(results, scenario.words, used, here)) {
          await page.keyboard.press('Escape');
          await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
          note(1, 'Escape (no result)');
          await settle(page, NOT_READY);
        }
        continue;
      }
      if (await scroll('scroll')) continue;
      record.stuck = `nothing on screen carries ${scenario.words.map((w) => `"${w}"`).join(', ')} at the end of ${pathNow()}`;
      break;
    }
    if (!record.reached && !record.stuck) record.stuck = `not there after ${MAX_ACTIONS} actions`;
    const shift = await shiftSoFar(page);
    record.layout_shift = Number.isFinite(shift) ? Math.round(shift * 1000) / 1000 : Number.NaN;
  } catch (error) {
    record.stuck = error instanceof Error ? error.message.split('\n')[0]! : String(error);
  }
  return record;
}
