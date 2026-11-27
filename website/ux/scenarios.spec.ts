// L3, the screen-only user: every scenario at each gating viewport in each theme, from the landing
// page. Gates 3 and 4: the user reaches the scenario's truth (its page, with its text in view) using
// only visible controls found by role and name. Gate 5's keyboard half: the search is reached and
// used by keyboard alone. The worst actions, looks and layout shift of each scenario go to the
// ratchet.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { BASE, GATING, NOT_READY, ORIGIN, OUT, SCENARIOS, THEMES } from './adapter.ts';
import { play } from './bot.ts';
import { MEASURE } from './reporter.ts';
import { assertTheme, hermetic, prepare, settle } from './page.ts';

test.beforeEach(async ({ context }) => hermetic(context, ORIGIN));

for (const scenario of SCENARIOS) {
  for (const theme of THEMES) {
    for (const viewport of GATING) {
      test(`L3 ${scenario.id} ${viewport.name} ${theme.name}`, async ({ page }) => {
        const run = await play(page, scenario, viewport, theme);
        const id = `${scenario.id}--${viewport.name}--${theme.name}`;
        mkdirSync(join(OUT, 'plays'), { recursive: true });
        mkdirSync(MEASURE, { recursive: true });
        writeFileSync(join(OUT, 'plays', `${id}.json`), JSON.stringify(run, null, 1));
        writeFileSync(
          join(MEASURE, `scenario-${id}.json`),
          JSON.stringify({ [scenario.id]: { 'screen_only.actions': run.actions, 'screen_only.looks': run.looks, layout_shift: run.layout_shift } }),
        );
        expect(run.reached, `"${scenario.question}" not reached: ${run.stuck ?? 'no reason'}; path: ${run.path.join(' → ') || '(none)'}`).toBe(true);
      });
    }
  }
}

/** Most Tab presses from the top of the landing page to the search field. */
const MAX_TABS = 30;

for (const theme of THEMES) {
  for (const viewport of GATING) {
    test(`L3 keyboard search ${viewport.name} ${theme.name}`, async ({ page }) => {
      await prepare(page, viewport, theme);
      await page.goto(BASE);
      await settle(page, NOT_READY);
      await assertTheme(page, theme);
      // The field reached by Tab shows that it has the focus (case first written by an adversary round).
      const look = () =>
        page.evaluate(() => {
          const s = getComputedStyle(document.querySelector('input.navbar__search-input')!);
          return [s.outlineStyle, s.outlineWidth, s.outlineColor, s.boxShadow, s.borderColor, s.borderWidth, s.backgroundColor].join('|');
        });
      const unfocused = await look();
      let tabs = 0;
      for (; tabs < MAX_TABS; tabs += 1) {
        await page.keyboard.press('Tab');
        if (await page.evaluate(() => document.activeElement?.matches('input.navbar__search-input') ?? false)) break;
      }
      expect(tabs, `the search field is reached within ${MAX_TABS} Tab presses`).toBeLessThan(MAX_TABS);
      expect(await look(), 'the focused search field looks the same as the unfocused one').not.toBe(unfocused);
      await page.keyboard.type('formation');
      await page.waitForSelector('[role=option]', { timeout: 5000 });
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('Enter');
      await page.waitForURL((url) => url.pathname.startsWith('/weft/docs/'), { timeout: 5000 });
      expect(new URL(page.url()).pathname).toMatch(/^\/weft\/docs\//);
    });
  }
}
