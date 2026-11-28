// Both consoles on fixture data: every page of both documents loads with every section read
// answered, has no axe violation (WCAG 2 A and AA), and has no horizontal overflow at 390 px.

import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { ALL } from './pages.ts';

async function settled(page: Page) {
  await expect(page.locator('#page-title')).toBeVisible();
  await expect(page.locator('[aria-busy="true"], .skeleton')).toHaveCount(0);
}

async function axe(page: Page) {
  const r = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
  return r.violations.map((v) => `${v.id}: ${v.help} (${v.nodes.slice(0, 3).map((n) => n.target.join(' ')).join(' | ')})`);
}

for (const c of ALL) {
  test(`${c.app} ${c.page}`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto(c.url);
    await settled(page);
    await expect(page.getByText('This section could not load')).toHaveCount(0);
    await expect(page.getByText('Not drawn')).toHaveCount(0);
    await expect(page.getByText('Host component missing')).toHaveCount(0);
    expect(await axe(page)).toEqual([]);
    await page.setViewportSize({ width: 390, height: 844 });
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow, 'horizontal overflow at 390 px').toBeLessThanOrEqual(0);
    expect(errors).toEqual([]);
  });
}

test('dark theme: home pages have no axe violation', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  for (const url of ['/operator/?latency=0#/overview', '/account/?latency=0#/home', '/account/?latency=0#/instances/detail/dbb0fcea-c300-4eae-98b3-a65c4ed77007']) {
    await page.goto(url);
    await settled(page);
    expect(await page.evaluate(() => document.documentElement.dataset.theme)).toBe('dark');
    expect(await axe(page)).toEqual([]);
  }
});

test('a row action form sends its command with the opener values', async ({ page }) => {
  await page.goto('/operator/?latency=0#/node_pools');
  await settled(page);
  await page.getByRole('button', { name: /^Actions for / }).first().click();
  await page.getByRole('menuitem', { name: 'Edit' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: /^(Save|Update|Edit)/ }).click();
  const sent = await page.evaluate(() => (window as unknown as { __weftCommands?: { command: string; input: Record<string, unknown> }[] }).__weftCommands ?? []);
  expect(sent.length).toBe(1);
  expect(sent[0].input.node_pool_id).toBeTruthy();
});

test('keyboard: the navigation drawer opens and closes at 390 px', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/account/?latency=0#/home');
  await settled(page);
  const toggle = page.getByRole('button', { name: 'Open navigation' });
  await toggle.focus();
  await page.keyboard.press('Enter');
  const nav = page.getByRole('navigation', { name: 'Main' });
  await expect(nav).toBeVisible();
  await nav.getByRole('link', { name: 'Instances' }).click();
  await settled(page);
  await expect(page.locator('#page-title')).toContainText('Instances');
  await expect(nav).toBeHidden();
});

test('search narrows a list and lives in the URL', async ({ page }) => {
  await page.goto('/operator/?latency=0#/nodes');
  await settled(page);
  const rows = page.locator('table tbody tr');
  const before = await rows.count();
  await page.getByRole('searchbox').fill('Draining');
  await expect.poll(() => rows.count()).toBeLessThan(before);
  expect(page.url()).toContain('search=Draining');
});
