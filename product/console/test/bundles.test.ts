// Both documents load into bundles, and everything they name is there: a fixture for every view
// they read, every command in the model, every form field an input of its command, a component for
// every host slot, every navigation entry a page.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { APPS, UI, compileModel, loadBundle } from '../build/bundle.ts';
import { FixtureServer } from '../src/renderer/fixtures.ts';
import { accountHosts, operatorHosts } from '../src/hosts/index.tsx';
import type { Bundle, Json, Obj } from '../src/renderer/types.ts';

const ir = compileModel();
const HOSTS = { operator: operatorHosts, account: accountHosts };

function walk(v: Json, f: (o: Obj) => void) {
  if (Array.isArray(v)) v.forEach((x) => walk(x, f));
  else if (v && typeof v === 'object') {
    f(v);
    Object.values(v).forEach((x) => walk(x, f));
  }
}

function viewsRead(b: Bundle): Set<string> {
  const out = new Set<string>();
  walk(b.doc.pages, (o) => {
    if (o.reads && typeof o.reads === 'object' && typeof o.reads.view === 'string') out.add(o.reads.view);
    if (o.label_from?.view) out.add(o.label_from.view);
    if (o.export?.reads) out.add(o.export.reads);
  });
  return out;
}

describe.each(APPS)('%s console', (app) => {
  const b = loadBundle(app, [], ir);

  it('loads as ess-ui/1 with pages, fixtures and an actor', () => {
    expect(b.doc.format).toBe('ess-ui/1');
    expect(b.doc.app).toBe(app);
    expect(Object.keys(b.doc.pages).length).toBeGreaterThan(8);
    expect(Object.keys(b.fixtures.views).length).toBeGreaterThan(20);
    expect(b.actor.principal_id).toBeTruthy();
  });

  it('every view it reads has a fixture', () => {
    const server = new FixtureServer(b.fixtures);
    const missing = [...viewsRead(b)].filter((v) => !server.has(v));
    expect(missing).toEqual([]);
  });

  it('every command it sends is in the model, and every form field is an input of its command', () => {
    const problems: string[] = [];
    walk(b.doc.pages, (o) => {
      if (typeof o.does !== 'string') return;
      const c = b.model.commands[o.does];
      if (!c) { problems.push(`${o.does}: not in the model`); return; }
      if (o.component === 'form') {
        for (const f of o.fields ?? []) if (!c.inputs.some((i) => i.name === f.field)) problems.push(`${o.does}: no input ${f.field}`);
      }
    });
    expect(problems).toEqual([]);
  });

  it('every host slot has a component', () => {
    const hosts = new Set<string>();
    walk(b.doc.pages, (o) => { if (typeof o.host === 'string') hosts.add(o.host); });
    expect([...hosts].filter((h) => !(h in HOSTS[app]))).toEqual([]);
    expect(hosts.size).toBeGreaterThan(0);
  });

  it('ui/<app>.hosts (what `ess ui check --hosts` reads) lists the registered host components', () => {
    const listed = readFileSync(join(UI, `${app}.hosts`), 'utf8').split('\n').map((l) => l.replace(/#.*/, '').trim()).filter(Boolean);
    expect(listed.sort()).toEqual(Object.keys(HOSTS[app]).sort());
  });

  it('every navigation entry and every navigate target is a page', () => {
    const pages = new Set(Object.keys(b.doc.pages));
    const named: string[] = [String(b.doc.navigation.home), ...(b.doc.navigation.hidden ?? [])];
    for (const s of b.doc.navigation.sections ?? []) named.push(...s.pages);
    walk(b.doc.pages, (o) => { if (o.navigate?.to) named.push(o.navigate.to); });
    expect(named.filter((p) => !pages.has(p))).toEqual([]);
  });

  it('every page names one actor of the model', () => {
    const actors = Object.keys(ir.actors ?? {});
    const named = new Set(Object.values(b.doc.pages).map((p) => p.actor));
    for (const a of named) expect(actors).toContain(a);
  });
});

it('the two consoles have their own accent', () => {
  const [op, ac] = APPS.map((a) => loadBundle(a, [], ir).doc.tokens?.color?.accent);
  expect(op).toBeTruthy();
  expect(op).not.toBe(ac);
});
