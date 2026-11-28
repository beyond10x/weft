// Screenshots of the main screens of both consoles at 1280 and 390 px, light and dark, into
// .out/shots/<app>-<page>-<width>-<theme>.png. Not part of the gate: `npm run shots`.

import { expect, test } from '@playwright/test';
import { ALL } from './pages.ts';

const MAIN: Record<string, string[]> = {
  operator: ['overview', 'nodes', 'nodes.detail', 'node_pools', 'scheduling', 'tenants.detail', 'quotas'],
  account: ['home', 'formations', 'formations.detail', 'workloads.detail', 'instances', 'instances.detail', 'releases.detail', 'catalog', 'usage'],
};

for (const c of ALL.filter((p) => MAIN[p.app].includes(p.page))) {
  for (const width of [1280, 390]) {
    for (const theme of ['light', 'dark'] as const) {
      test(`${c.app} ${c.page} ${width} ${theme}`, async ({ page }) => {
        await page.emulateMedia({ colorScheme: theme });
        await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
        const url = c.page === 'quotas' ? c.url.replace('#/quotas', '#/quotas?tenant=a38fd8ba-fa25-42c4-9a58-974e92dcf880') : c.url;
        await page.goto(url);
        await expect(page.locator('#page-title')).toBeVisible();
        await expect(page.locator('[aria-busy="true"], .skeleton')).toHaveCount(0);
        await page.screenshot({ path: `.out/shots/${c.app}-${c.page.replace(/\./g, '-')}-${width}-${theme}.png`, fullPage: true });
      });
    }
  }
}
