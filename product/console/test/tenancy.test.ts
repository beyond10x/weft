// The account console shows one account. Every read of a view whose rows belong to an account
// (the view has an `account_id` field) is scoped to the signed-in actor's account: it binds
// `account_id: actor.account_id`. A read keyed only by an id taken from the URL (a page param or a
// URL-stored state such as `?formation=`) would otherwise answer another account's rows.
//
// Where the model offers no account-scoped form of a view (no `account_id` param), the document
// cannot scope the read. Those reads are listed in `unscoped-reads.txt`, each with its reason; the
// list only shrinks: a read missing from it fails, a listed read that is scoped (or gone) fails, and
// a listed read whose view gained an `account_id` param fails (bind it then).
//
// `label_from` reads its view without params (ess-ui/1), and `ess ui check` refuses one whose view
// requires a param; a `label_from` of an account-owned view without an `account_id` param reads
// every account's rows and counts as unscoped. A label from a view that takes `account_id` is a host
// slot that names the scoped read (`AlertRuleName` on home/alerts).

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { compileModel, loadBundle } from '../build/bundle.ts';
import type { Json, Obj } from '../src/renderer/types.ts';

const ir = compileModel();
const LIST = fileURLToPath(new URL('./unscoped-reads.txt', import.meta.url));
const REASON = 'model view has no account_id param';

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

/** Every read of an account-owned view that is not bound to the actor's account, as `<path>: <View>(<params>)`. */
function unscopedReads(): string[] {
  const b = loadBundle('account', [], ir);
  const out: string[] = [];
  for (const [name, page] of Object.entries(b.doc.pages)) {
    walk(page as Json, name, (o, path) => {
      const r = o.reads && typeof o.reads === 'object' ? o.reads : undefined;
      if (r?.view && fieldsOf(r.view).includes('account_id') && r.params?.account_id !== 'actor.account_id') {
        out.push(`${path}: ${r.view}(${Object.entries(r.params ?? {}).map(([k, v]) => `${k}: ${v}`).join(', ')})`);
      }
      const l = o.label_from;
      if (l?.view && fieldsOf(l.view).includes('account_id') && !paramsOf(l.view).includes('account_id')) {
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

  it('every read of an account-owned view binds account_id to actor.account_id, or is listed in unscoped-reads.txt', () => {
    const names = new Set(list.map((x) => x.read));
    expect(found.filter((r) => !names.has(r)), 'unscoped reads not in test/unscoped-reads.txt: bind account_id: actor.account_id').toEqual([]);
  });

  it('the list only shrinks: every listed read is still in the document and still unscoped', () => {
    const now = new Set(found);
    expect(list.map((x) => x.read).filter((r) => !now.has(r)), 'listed reads that are scoped or gone: remove them from test/unscoped-reads.txt').toEqual([]);
  });

  it('every listed read gives its reason, and the reason holds in the model', () => {
    const wrong: string[] = [];
    for (const { read, reason } of list) {
      const view = read.match(/: ([\w.]+)\(/)?.[1] ?? '';
      if (reason !== REASON) wrong.push(`${read}: reason "${reason}", want "${REASON}"`);
      if (!viewOf(view)) wrong.push(`${read}: ${view} is no view of the model`);
      else if (paramsOf(view).includes('account_id')) wrong.push(`${read}: ${view} now takes account_id; bind it and remove the line`);
    }
    expect(wrong).toEqual([]);
  });
});
