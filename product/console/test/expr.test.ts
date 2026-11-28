import { describe, expect, it } from 'vitest';
import { evaluate, holds, parse, reads, value, values } from '../src/renderer/expr.ts';
import type { Scope } from '../src/renderer/expr.ts';

const s: Scope = {
  state: { search: 'gpu', tenant: 't1', window: { preset: 'last_30d', from: 'a' } },
  params: { node_id: 'n1' },
  row: { node_id: 'n1', state: 'Ready', zone: 'zone-a', tags: ['gpu', 'edge'], n: 3 },
  actor: { account_id: 'acc' },
  section: (name) => (name === 'summary' ? { row: { state: 'Draining' } } : undefined),
};

describe('expressions', () => {
  it('reads paths of every root', () => {
    expect(value('state.search', s)).toBe('gpu');
    expect(value('params.node_id', s)).toBe('n1');
    expect(value('actor.account_id', s)).toBe('acc');
    expect(value('state.window.from', s)).toBe('a');
    expect(value('section.summary.row.state', s)).toBe('Draining');
    expect(value('state.missing', s)).toBeUndefined();
  });

  it('compares with bare words as literals', () => {
    expect(holds('row.state == Ready', s)).toBe(true);
    expect(holds('row.state != Ready', s)).toBe(false);
    expect(holds('row.state in [Ready, Cordoned]', s)).toBe(true);
    expect(holds('row.state in [Gone]', s)).toBe(false);
    expect(holds('row.node_id == params.node_id', s)).toBe(true);
  });

  it('combines with not, and, or', () => {
    expect(holds('not row.state == Gone', s)).toBe(true);
    expect(holds('row.state == Ready and row.zone == zone-a', s)).toBe(true);
    expect(holds('row.state == Gone or row.zone == zone-a', s)).toBe(true);
    expect(holds('row.state == Gone and row.zone == zone-a', s)).toBe(false);
  });

  it('`row contains` matches any field by words, and an empty search keeps every row', () => {
    expect(holds('row contains state.search', s)).toBe(true);
    expect(holds('row contains state.search', { ...s, state: { search: 'nomatch' } })).toBe(false);
    expect(holds('row contains state.search', { ...s, state: { search: '' } })).toBe(true);
  });

  it('leaves literal text alone', () => {
    expect(parse('Shown to the owners of the evicted workloads.')).toBeNull();
    expect(value('No node is draining.', s)).toBe('No node is draining.');
    expect(holds(undefined, s)).toBe(true);
  });

  it('evaluates maps of binds and lists the paths it reads', () => {
    expect(values({ node_id: 'row.node_id', account_id: 'actor.account_id' }, s)).toEqual({ node_id: 'n1', account_id: 'acc' });
    expect(reads('row.state == Ready and state.search == x')).toEqual(['row.state', 'state.search']);
    const e = parse('row.n');
    expect(e && evaluate(e, s)).toBe(3);
  });
});
