import { describe, expect, it } from 'vitest';
import { actionName, field, findEnum, mergeMaps, mergeNamed, normalize } from '../src/renderer/load.ts';
import type { ModelMeta } from '../src/renderer/types.ts';

const doc = (pages: Record<string, unknown>, extra: Record<string, unknown> = {}) => ({
  format: 'ess-ui/1', app: 't', model: 'm', shells: {}, navigation: { home: Object.keys(pages)[0], sections: [] }, pages, ...extra,
});

describe('page kinds', () => {
  it('a list page inherits the filter bar, the list, URL state and the header', () => {
    const d = normalize(doc({ nodes: { kind: 'list_page', title: 'Nodes', sections: [{ name: 'list', component: 'collection', reads: 'v.Nodes' }] } }));
    const p = d.pages.nodes;
    expect(p.sections.map((s) => s.name)).toEqual(['filters', 'list']);
    expect(p.sections[1].reads).toEqual({ view: 'v.Nodes' });
    expect(p.state?.search?.store).toBe('url');
    expect(p.header).toMatchObject({ title: 'Nodes', total: 'list', filters: 'filters' });
  });

  it('`remove: true` drops an inherited section and `null` drops a header key', () => {
    const d = normalize(doc({ q: { kind: 'list_page', title: 'Q', header: { filters: null }, sections: [{ name: 'filters', remove: true }] } }));
    expect(d.pages.q.sections.map((s) => s.name)).toEqual(['list']);
    expect(d.pages.q.header).not.toHaveProperty('filters');
  });

  it('a dashboard page replaces its board', () => {
    const d = normalize(doc({ o: { kind: 'dashboard_page', title: 'O', sections: [{ name: 'board', remove: true }, { name: 'n', component: 'metric', reads: 'v.N', aggregate: 'count' }] } }));
    expect(d.pages.o.sections.map((s) => [s.name, s.component])).toEqual([['n', 'metric']]);
  });

  it('a page kind of the document extends a builtin', () => {
    const d = normalize(doc({ a: { kind: 'tenant_list', title: 'A', sections: [] } }, { page_kinds: { tenant_list: { extends: 'list_page', header: { help: { text: 'h' } } } } }));
    expect(d.pages.a.sections.map((s) => s.name)).toEqual(['filters', 'list']);
    expect(d.pages.a.header?.help).toEqual({ text: 'h' });
  });

  it('rejects another format', () => {
    expect(() => normalize({ format: 'ess-ui/0' })).toThrow(/ess-ui\/1/);
  });
});

describe('short forms', () => {
  it('a field written as a name', () => {
    expect(field('zone')).toMatchObject({ field: 'zone', name: 'zone' });
    expect(field({ field: 'state', as: 'badge' })).toMatchObject({ field: 'state', name: 'state', as: 'badge' });
  });

  it('an action is named by what it does', () => {
    expect(actionName({ name: 'x', does: 'a.B' })).toBe('x');
    expect(actionName({ opens: 'create' })).toBe('create');
  });

  it('maps merge deeply and named lists merge by name', () => {
    expect(mergeMaps({ a: { b: 1, c: 2 }, d: 1 }, { a: { c: 3 }, d: null })).toEqual({ a: { b: 1, c: 3 } });
    expect(mergeNamed([{ name: 'a', x: 1 }, { name: 'b' }], [{ name: 'a', y: 2 }, { name: 'c' }]).map((s) => s.name)).toEqual(['a', 'b', 'c']);
  });
});

describe('enums by name', () => {
  const model: ModelMeta = { system: 'm', enums: { 'm.infra.NodeRole': ['Control', 'Worker'], 'm.x.Kind': ['A'], 'm.y.Kind': ['B'] }, commands: {}, views: {}, actors: {} };
  it('qualified, by unique suffix, and not when the suffix is ambiguous', () => {
    const d = normalize(doc({ p: { kind: 'detail_page', title: 'P', sections: [] } }));
    expect(findEnum(model, d, 'm.infra.NodeRole')).toEqual(['Control', 'Worker']);
    expect(findEnum(model, d, 'infra.NodeRole')).toEqual(['Control', 'Worker']);
    expect(findEnum(model, d, 'Kind')).toBeUndefined();
  });
});
