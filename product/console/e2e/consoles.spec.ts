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

// ── Adversary cases (story console-ui) ─────────────────────────────────────────────────────

/**
 * Every element in the page whose content is wider than its box at the current width: content cut
 * off (`overflow` hidden or clip, not an ellipsis with the full text in a title), or a scroll box
 * a reader cannot see or reach (no visible scrollbar and no drawn affordance, or not focusable).
 */
async function clipped(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    for (const el of Array.from(document.querySelectorAll<HTMLElement>('main *'))) {
      if (el.clientWidth === 0 || el.scrollWidth <= el.clientWidth + 1) continue;
      if (el.closest('.sr-only')) continue;
      const cs = getComputedStyle(el);
      const name = el.getAttribute('aria-label') ?? el.closest('section')?.querySelector('h2,h3')?.textContent ?? el.className;
      const what = `${name} (${el.tagName.toLowerCase()}.${el.className}): ${el.scrollWidth} px of content in ${el.clientWidth} px`;
      if (cs.overflowX === 'hidden' || cs.overflowX === 'clip') {
        if (cs.textOverflow === 'ellipsis' && el.closest('[title]')) continue;
        out.push(`${what}, cut off`);
      } else if (cs.overflowX === 'auto' || cs.overflowX === 'scroll') {
        const bar = el.offsetHeight - el.clientHeight - parseFloat(cs.borderTopWidth) - parseFloat(cs.borderBottomWidth);
        const drawn = el.matches('[data-overflow], .scroll-hint, .has-overflow');
        if (bar <= 0 && !drawn) out.push(`${what}, scrolls with no visible affordance`);
        if (el.tabIndex < 0) out.push(`${what}, scroll box not keyboard reachable`);
      }
    }
    return out;
  });
}

for (const app of ['operator', 'account'] as const) {
  test(`layout at 1280: ${app} pages cut off no table content and show where a table scrolls`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    const found: string[] = [];
    for (const c of ALL.filter((p) => p.app === app)) {
      await page.goto(c.url);
      await settled(page);
      for (const f of await clipped(page)) found.push(`${c.page}: ${f}`);
    }
    expect(found).toEqual([]);
  });
}

test('names, not ids: account home names the alert rule', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/account/?latency=0#/home');
  await settled(page);
  const card = (title: string) => page.locator('section.card', { has: page.getByRole('heading', { name: new RegExp(`^${title}`) }) });
  await expect(card('Alerts').locator('.label-from').first()).toBeVisible();
  expect(await card('Alerts').locator('.id-text').count(), 'Alerts: the Rule column shows alert_rule_id, not the rule name').toBe(0);
});

test('names, not ids: account home names the workload of a service', async ({ page }) => {
  test.skip(true, 'model gap: no view of weft.workloads gives a workload name for a workload_id without a formation (WorkloadList takes formation_id, WorkloadDetail one workload_id), so label_from has nothing to read; a weft model story');
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto('/account/?latency=0#/home');
  await settled(page);
  const card = (title: string) => page.locator('section.card', { has: page.getByRole('heading', { name: new RegExp(`^${title}`) }) });
  expect(await card('Services').locator('.id-text').count(), 'Services: the Workload column shows workload_id, not a name').toBe(0);
});

test('grants: a Deployer-only actor sees no page and no action its grants do not include', async ({ page }) => {
  await page.goto('/account/?latency=0#/formations');
  await settled(page);
  await expect(page.locator('.page-actions').getByRole('button', { name: 'New formation' }), 'the full actor (deployer, account_admin) may create').toBeVisible();

  await page.goto('/account/?latency=0&actor=deployer#/home');
  await settled(page);
  const nav = page.getByRole('navigation', { name: 'Main' });
  await expect(nav.getByRole('link', { name: 'Workloads' })).toBeVisible();
  await expect(nav.getByRole('link', { name: 'Formations' }), 'formations is an AccountAdmin page').toHaveCount(0);
  await expect(nav.getByRole('link', { name: 'Usage and cost' }), 'usage is an AccountAdmin page').toHaveCount(0);

  await page.goto('/account/?latency=0&actor=deployer#/formations');
  await expect(page.locator('#page-title')).toBeVisible();
  await expect(page.getByRole('button', { name: 'New formation' }), 'CreateFormation is not granted to Deployer').toHaveCount(0);
  await expect(page.getByRole('button', { name: /^Actions for / })).toHaveCount(0);
});

test('keyboard: the navigation drawer at 390 px takes focus, closes on Escape and gives focus back', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/account/?latency=0#/home');
  await settled(page);
  const toggle = page.getByRole('button', { name: 'Open navigation' });
  await toggle.focus();
  await page.keyboard.press('Enter');
  const nav = page.getByRole('navigation', { name: 'Main' });
  await expect(nav).toBeVisible();
  expect(await page.evaluate(() => !!document.activeElement?.closest('.sidebar')), 'focus moves into the open drawer').toBe(true);
  await page.keyboard.press('Escape');
  await expect(nav, 'Escape closes the drawer').toBeHidden();
  await expect(toggle, 'focus returns to the toggle').toBeFocused();
});

test('keyboard: a dialog keeps focus inside, closes on Escape and gives focus back to its opener', async ({ page }) => {
  await page.goto('/operator/?latency=0#/node_pools');
  await settled(page);
  const opener = page.locator('.page-actions').getByRole('button', { name: 'New node pool' });
  await opener.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  for (let i = 0; i < 30; i++) {
    await page.keyboard.press('Tab');
    expect(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]')), `Tab ${i + 1} stays in the dialog`).toBe(true);
  }
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(opener).toBeFocused();
});

test('theme: the theme chosen in the account menu survives a reload', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/account/?latency=0#/home');
  await settled(page);
  await page.getByRole('button', { name: /on-call-engineer/ }).click();
  await page.getByRole('menuitemradio', { name: 'Dark theme' }).click();
  await expect.poll(() => page.evaluate(() => document.documentElement.dataset.theme)).toBe('dark');
  await page.reload();
  await settled(page);
  expect(await page.evaluate(() => document.documentElement.dataset.theme)).toBe('dark');
});
