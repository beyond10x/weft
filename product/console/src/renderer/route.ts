// Routes derived from page names (ess-ui/1: "its route is derived from the name"): page
// `nodes.detail` with params `{node_id}` is `#/nodes/detail/<node_id>`; page state in the URL is the
// query (`#/nodes?search=gpu`). Hash routes keep the build servable from any static host.

import type { Doc, Json } from './types.ts';

export interface Route {
  page: string;
  params: Record<string, string>;
  query: Record<string, string>;
}

export function href(doc: Doc, page: string, params: Record<string, Json> = {}, query: Record<string, Json> = {}): string {
  const p = doc.pages[page];
  const segs = page.split('.').map(encodeURIComponent);
  for (const name of Object.keys(p?.params ?? {})) segs.push(encodeURIComponent(String(params[name] ?? '')));
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(query)) {
    if (v === undefined || v === null || v === '') continue;
    q.set(k, typeof v === 'object' ? JSON.stringify(v) : String(v));
  }
  const qs = q.toString();
  return `#/${segs.join('/')}${qs ? `?${qs}` : ''}`;
}

export function parseHash(doc: Doc, hash: string): Route {
  const raw = hash.replace(/^#\/?/, '');
  const [path, qs = ''] = raw.split('?');
  const segs = path.split('/').filter(Boolean).map(decodeURIComponent);
  const query = Object.fromEntries(new URLSearchParams(qs));
  const home = String(doc.navigation.home);
  if (!segs.length) return { page: home, params: {}, query };
  // The longest page name that is a prefix of the path and whose param count fits the rest.
  let best: Route | undefined;
  let bestLen = -1;
  for (const [name, page] of Object.entries(doc.pages)) {
    const ns = name.split('.');
    const pnames = Object.keys(page.params ?? {});
    if (ns.length + pnames.length !== segs.length) continue;
    if (!ns.every((s, i) => s === segs[i])) continue;
    if (ns.length > bestLen) {
      bestLen = ns.length;
      best = { page: name, params: Object.fromEntries(pnames.map((p, i) => [p, segs[ns.length + i]])), query };
    }
  }
  return best ?? { page: '__not_found__', params: { path }, query };
}
