// The account console shows one account. Every read of a view whose rows belong to an owner (the
// view has a `scope_id` or an `account_id` field, OWNERS) is scoped to the signed-in actor: it
// binds `<owner>: actor.<owner>` for each owner field the view has. The model keys tenant data by
// the account's scope (`scope_id`, weft.scope); `account_id` stays for the views that still carry
// it. A read keyed only by an id taken from the URL (a page param or a URL-stored state such as
// `?formation=`) would otherwise answer another account's rows.
//
// Where the model offers no scoped form of a view (no param of the owner field), the document
// cannot scope the read. Those reads are listed in `unscoped-reads.txt`, each with its reason; the
// list only shrinks: a read missing from it fails, a listed read that is scoped (or gone) fails, and
// a listed read whose view gained the owner param fails (bind it then).
//
// `label_from` reads its view without params (ess-ui/1), and `ess ui check` refuses one whose view
// requires a param; a `label_from` of an owned view without the owner param reads every account's
// rows and counts as unscoped. A label from a view that takes the owner param is a host slot that
// names the scoped read (`AlertRuleName` on home/alerts).

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { compileModel, loadBundle } from '../build/bundle.ts';
import type { Json, Obj } from '../src/renderer/types.ts';

const ir = compileModel();
const LIST = fileURLToPath(new URL('./unscoped-reads.txt', import.meta.url));
const OWNERS = ['scope_id', 'account_id'];
const reasonFor = (owner: string) => `model view has no ${owner} param`;

function walk(v: Json, path: string, f: (o: Obj, path: string) => void) {
  if (Array.isArray(v)) v.forEach((x, i) => walk(x, `${path}[${x && typeof x === 'object' && typeof x.name === 'string' ? x.name : i}]`, f));
  else if (v && typeof v === 'object') {
    f(v, path);
    for (const [k, x] of Object.entries(v)) walk(x, `${path}.${k}`, f);
  }
}

const viewOf = (name: string): Obj | undefined => (ir.views as Obj)[name];
const paramsOf = (name: string): string[] => (viewOf(name)?.params ?? []).map((p: Obj) => p.name);
const fieldsOf = (name: string): string[] => (viewOf(name)?.fields ?? []).map((x: Obj) => x.name);
/** The owner fields of a view: which of OWNERS its rows carry. */
const ownersOf = (name: string): string[] => OWNERS.filter((o) => fieldsOf(name).includes(o));

/** Every read of an owned view that is not bound to the actor, as `<path>: <View>(<params>)`. */
function unscopedReads(): string[] {
  const b = loadBundle('account', [], ir);
  const out: string[] = [];
  for (const [name, page] of Object.entries(b.doc.pages)) {
    walk(page as Json, name, (o, path) => {
      const r = o.reads && typeof o.reads === 'object' ? o.reads : undefined;
      if (r?.view && ownersOf(r.view).some((k) => r.params?.[k] !== `actor.${k}`)) {
        out.push(`${path}: ${r.view}(${Object.entries(r.params ?? {}).map(([k, v]) => `${k}: ${v}`).join(', ')})`);
      }
      const l = o.label_from;
      if (l?.view && ownersOf(l.view).some((k) => !paramsOf(l.view).includes(k))) {
        out.push(`${path}.label_from: ${l.view}()`);
      }
    });
  }
  return out;
}

/** The committed list: `<read> | <reason>` per line; `#` starts a comment. */
function listed(): { read: string; reason: string }[] {
  return readFileSync(LIST, 'utf8')
    .split('\n')
    .map((l) => l.replace(/#.*/, '').trim())
    .filter(Boolean)
    .map((l) => {
      const at = l.lastIndexOf(' | ');
      return at < 0 ? { read: l, reason: '' } : { read: l.slice(0, at).trim(), reason: l.slice(at + 3).trim() };
    });
}

describe('account console tenancy', () => {
  const found = unscopedReads();
  const list = listed();

  it('the account console reads views keyed by scope_id', () => {
    expect(found.length + list.length, 'the walk finds the owned reads at all').toBeGreaterThan(0);
    const b = loadBundle('account', [], ir);
    let scoped = 0;
    for (const page of Object.values(b.doc.pages)) walk(page as Json, '', (o) => { if (o.reads?.params?.scope_id === 'actor.scope_id') scoped++; });
    expect(scoped).toBeGreaterThan(10);
  });

  it('every read of an owned view binds its owner fields to the actor, or is listed in unscoped-reads.txt', () => {
    const names = new Set(list.map((x) => x.read));
    expect(found.filter((r) => !names.has(r)), 'unscoped reads not in test/unscoped-reads.txt: bind scope_id: actor.scope_id').toEqual([]);
  });

  it('the list only shrinks: every listed read is still in the document and still unscoped', () => {
    const now = new Set(found);
    expect(list.map((x) => x.read).filter((r) => !now.has(r)), 'listed reads that are scoped or gone: remove them from test/unscoped-reads.txt').toEqual([]);
  });

  it('every listed read gives its reason, and the reason holds in the model', () => {
    const wrong: string[] = [];
    for (const { read, reason } of list) {
      const view = read.match(/: ([\w.]+)\(/)?.[1] ?? '';
      if (!viewOf(view)) { wrong.push(`${read}: ${view} is no view of the model`); continue; }
      const missing = ownersOf(view).filter((k) => !paramsOf(view).includes(k));
      if (!missing.length) wrong.push(`${read}: ${view} now takes ${ownersOf(view).join(', ')}; bind it and remove the line`);
      else if (reason !== reasonFor(missing[0])) wrong.push(`${read}: reason "${reason}", want "${reasonFor(missing[0])}"`);
    }
    expect(wrong).toEqual([]);
  });
});
