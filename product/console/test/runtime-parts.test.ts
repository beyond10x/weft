import { describe, expect, it } from 'vitest';
import { FixtureServer, NoFixture, matchParams } from '../src/renderer/fixtures.ts';
import * as fmt from '../src/renderer/format.ts';
import { href, parseHash } from '../src/renderer/route.ts';
import { cssVars, resolveTheme, themeCss, themeTokens } from '../src/renderer/theme.ts';
import { normalize } from '../src/renderer/load.ts';
import { windowRange, windowValue } from '../src/renderer/nodes/FilterBar.tsx';
import { commandInput, controlOf } from '../src/renderer/nodes/Form.tsx';
import { aggregate, formatMetric } from '../src/renderer/nodes/Metric.tsx';

describe('fixtures', () => {
  const server = new FixtureServer({
    views: { 'v.Nodes': { rows: [{ node_id: 'a', zone: 'z1' }, { node_id: 'b', zone: 'z2' }] } },
    derived: { 'v.NodeById': { by_id_from: 'v.Nodes', key: 'node_id' }, 'v.Same': { same_as: 'v.Nodes' } },
  });
  it('keeps rows whose fields equal the params, ignores params that name no field', () => {
    expect(server.read('v.Nodes', { zone: 'z2' }).rows).toEqual([{ node_id: 'b', zone: 'z2' }]);
    expect(server.read('v.Nodes', { from: '2026-01-01' }).rows).toHaveLength(2);
    expect(matchParams({ a: 1 }, { a: '1', b: null })).toBe(true);
  });
  it('answers derived views', () => {
    expect(server.read('v.NodeById', { node_id: 'a' }).rows).toEqual([{ node_id: 'a', zone: 'z1' }]);
    expect(server.read('v.Same').total).toBe(2);
  });
  it('a view without fixture fails the read', () => {
    expect(() => server.read('v.Missing')).toThrow(NoFixture);
  });
});

describe('formats', () => {
  it('bytes, durations, percent, numbers', () => {
    expect(fmt.bytes(1536)).toBe('1.5 KB');
    expect(fmt.duration(90, 's')).toBe('0:01:30');
    expect(fmt.durationText('30s')).toMatch(/30/);
    expect(fmt.percent(0.25)).toBe('25%');
    expect(fmt.number(12345)).toBe('12,345');
  });
  it('timestamps in UTC, short ids, words', () => {
    expect(fmt.timestamp('2026-11-27T14:02:00Z')).toContain('14:02');
    expect(fmt.shortId('531fdf41-b814-464f-a3d8-0b4f9ed7bce8')).toMatch(/^531fdf41/);
    expect(fmt.words('MicroVm')).toBe('Micro vm');
    expect(fmt.toneOf('Ready', { Ready: 'success' })).toBe('success');
    expect(fmt.toneOf('Other', { Ready: 'success' }, 'info')).toBe('info');
  });
});

describe('routes', () => {
  const d = normalize({
    format: 'ess-ui/1', app: 't', model: 'm', shells: {}, navigation: { home: 'nodes' },
    pages: {
      nodes: { kind: 'list_page', title: 'Nodes', sections: [] },
      'nodes.detail': { kind: 'detail_page', title: 'Node', params: { node_id: 'NodeId' }, sections: [] },
    },
  });
  it('page names and params make the hash, and back', () => {
    const h = href(d, 'nodes.detail', { node_id: 'abc' });
    expect(h).toBe('#/nodes/detail/abc');
    expect(parseHash(d, h)).toEqual({ page: 'nodes.detail', params: { node_id: 'abc' }, query: {} });
    expect(parseHash(d, href(d, 'nodes', {}, { search: 'gpu' }))).toEqual({ page: 'nodes', params: {}, query: { search: 'gpu' } });
    expect(parseHash(d, '').page).toBe('nodes');
    expect(parseHash(d, '#/nope').page).toBe('__not_found__');
  });
});

describe('themes', () => {
  const d = normalize({
    format: 'ess-ui/1', app: 't', model: 'm', shells: {}, navigation: { home: 'p' }, pages: { p: { kind: 'detail_page', title: 'P', sections: [] } },
    tokens: { color: { accent: '#111111' } }, themes: { system: {}, light: {}, dark: { color: { accent: '#eeeeee' } } }, theme: { default: 'system' },
  });
  it('each theme overrides the base tokens', () => {
    const t = themeTokens(d);
    expect(t.light.color.accent).toBe('#111111');
    expect(t.dark.color.accent).toBe('#eeeeee');
    expect(cssVars(t.dark)).toContain('--color-accent: #eeeeee;');
    expect(themeCss(d)).toContain(':root[data-theme="dark"]');
    expect(themeCss(d)).not.toContain('"system"');
  });
  it('system follows the color scheme, a chosen theme wins', () => {
    expect(resolveTheme(d, undefined, true)).toBe('dark');
    expect(resolveTheme(d, 'system', false)).toBe('light');
    expect(resolveTheme(d, 'light', true)).toBe('light');
  });
});

describe('window, form input, metrics', () => {
  it('window presets fill from and to', () => {
    const now = new Date('2026-11-28T10:00:00Z');
    expect(windowRange('this_month', now).from).toBe('2026-11-01T00:00:00.000Z');
    expect(windowRange('this_quarter', now).from).toBe('2026-10-01T00:00:00.000Z');
    expect(windowRange('last_30d', now).from).toBe('2026-10-29T10:00:00.000Z');
    expect(windowValue({ preset: 'last_30d' }).to).toBeTruthy();
    expect(windowValue({ preset: 'custom', from: 'x' })).toEqual({ preset: 'custom', from: 'x' });
  });

  it('a command input comes from the draft, then the opener, then the actor', () => {
    const inputs = [
      { name: 'node_id', type: 'NodeId', optional: false },
      { name: 'reason', type: 'string', optional: false },
      { name: 'account_id', type: 'AccountId', optional: false },
      { name: 'note', type: 'string', optional: true },
    ];
    expect(commandInput(inputs, { reason: 'maintenance', note: '' }, new Set(['reason', 'note']), { node_id: 'n1', reason: 'old' }, { account_id: 'acc' }, {}))
      .toEqual({ node_id: 'n1', reason: 'maintenance', account_id: 'acc' });
  });

  it('a hidden input the caller owns takes the actor value over a handed row or param', () => {
    const inputs = [
      { name: 'formation_id', type: 'FormationId', optional: false },
      { name: 'account_id', type: 'AccountId', optional: false },
      { name: 'tenant_id', type: 'TenantId', optional: false },
      { name: 'principal_id', type: 'PrincipalId', optional: true },
    ];
    const actor = { account_id: 'acc', tenant_id: 'ten', principal_id: 'me' };
    const handed = { formation_id: 'f1', account_id: 'other-acc', tenant_id: 'other-ten', principal_id: 'someone' };
    expect(commandInput(inputs, {}, new Set(), handed, actor, {}, new Set(['account_id', 'tenant_id', 'principal_id'])))
      .toEqual({ formation_id: 'f1', account_id: 'acc', tenant_id: 'ten', principal_id: 'me' });
    // Without a list, account_id and tenant_id are the caller's.
    expect(commandInput(inputs, {}, new Set(), handed, actor, {}))
      .toEqual({ formation_id: 'f1', account_id: 'acc', tenant_id: 'ten', principal_id: 'someone' });
    // An actor that carries no value (the operator acts on a tenant's row) leaves the handed one.
    expect(commandInput(inputs, {}, new Set(), { tenant_id: 'row-ten' }, { tenant_id: null, account_id: null }, {}))
      .toEqual({ tenant_id: 'row-ten' });
    // `bind` written in the document still wins.
    expect(commandInput(inputs, {}, new Set(), handed, actor, { account_id: 'bound' }).account_id).toBe('bound');
  });

  it('a control follows `as`, then the input type', () => {
    const e = (t: string) => (t === 'Role' ? ['A'] : undefined);
    expect(controlOf({ field: 'x', name: 'x', as: 'json' }, undefined, e)).toBe('json');
    expect(controlOf({ field: 'x', name: 'x' }, { name: 'x', type: 'integer', optional: false }, e)).toBe('number');
    expect(controlOf({ field: 'x', name: 'x' }, { name: 'x', type: 'Role', optional: false }, e)).toBe('enum');
    expect(controlOf({ field: 'x', name: 'x' }, { name: 'x', type: '[ZoneName]', optional: false }, e)).toBe('tags');
  });

  it('aggregates and formats metrics', () => {
    const rows = [{ cost: 2.5 }, { cost: 4 }, { cost: 'x' }];
    expect(aggregate(rows, 'count', undefined)).toBe(3);
    expect(aggregate(rows, 'sum', 'cost')).toBe(6.5);
    expect(aggregate([], 'sum', 'cost')).toBe(0);
    expect(aggregate(rows, 'max', 'cost')).toBe(4);
    expect(formatMetric(6.5, {})).toBe('6.5');
    expect(formatMetric(undefined, {})).toBe('—');
  });
});
