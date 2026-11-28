// Normalizes a parsed ess-ui/1 document for the renderer: every page merged over its page kind (the
// schema's inheritance rules), and the short forms the renderer meets expanded (a field written as a
// name, a read written as a view name, `title: from_page`). Pure: no file system, so the unit tests
// and the build plugin share it.

import type { Action, Doc, Field, Json, ModelMeta, Obj, Page, Reads, Section } from './types.ts';

/** The built-in page kinds of ess-ui/1 (`PageKind.builtins`). */
export const BUILTIN_KINDS: Record<string, Obj> = {
  list_page: {
    state: {
      search: { type: 'string', class: 'page_state', store: 'url', pinned: true },
      page: { type: 'integer', class: 'page_state', store: 'url', pinned: true, default: 1 },
      size: { type: 'integer', class: 'page_state', store: 'url', pinned: true, default: 25 },
      sort: { type: { optional: { record: { by: 'string', dir: { enum: ['asc', 'desc'] } } } }, class: 'page_state', store: 'url', pinned: true },
    },
    sections: [
      { name: 'filters', component: 'filter_bar', binds: ['state.search'], search: { binds: 'state.search' } },
      { name: 'list', component: 'collection' },
    ],
    header: { title: 'from_page', total: 'list', filters: 'filters' },
  },
  report_page: {
    state: {
      window: {
        type: { record: { from: 'timestamp', to: 'timestamp', preset: { enum: ['this_month', 'last_30d', 'this_quarter', 'custom'] } } },
        class: 'page_state',
        store: 'url',
        pinned: true,
        default: { preset: 'last_30d' },
      },
      page: { type: 'integer', class: 'page_state', store: 'url', pinned: true, default: 1 },
      size: { type: 'integer', class: 'page_state', store: 'url', pinned: true, default: 50 },
    },
    sections: [
      { name: 'filters', component: 'filter_bar', binds: ['state.window'], window: { binds: 'state.window' } },
      { name: 'list', component: 'collection' },
    ],
    header: { title: 'from_page', total: 'list', filters: 'filters' },
  },
  detail_page: { sections: [{ name: 'summary', component: 'record' }] },
  settings_page: { sections: [{ name: 'settings', component: 'form' }] },
  dashboard_page: { sections: [{ name: 'board', component: 'board' }] },
  editor_page: { sections: [{ name: 'editor', component: 'graph_editor' }] },
  form_page: { sections: [{ name: 'form', component: 'form' }] },
  static_page: { sections: [{ name: 'body', component: 'record' }] },
};

const isObj = (v: Json): v is Obj => v !== null && typeof v === 'object' && !Array.isArray(v);
const clone = <T>(v: T): T => (v === undefined ? v : JSON.parse(JSON.stringify(v)));

/** Deep map merge: page wins, `null` removes the inherited key, keys keep inherited order first. */
export function mergeMaps(base: Obj | undefined, over: Obj | undefined): Obj | undefined {
  if (over === undefined) return clone(base);
  if (base === undefined) return stripNulls(clone(over));
  const out: Obj = clone(base);
  for (const [k, v] of Object.entries(over)) {
    if (v === null) delete out[k];
    else if (isObj(v) && isObj(out[k])) out[k] = mergeMaps(out[k], v);
    else out[k] = clone(v);
  }
  return out;
}

function stripNulls(o: Obj): Obj {
  for (const k of Object.keys(o)) if (o[k] === null) delete o[k];
  return o;
}

/** Named-list merge (sections): inherited in their order, then new in page order; `{name, remove: true}` removes. */
export function mergeNamed(base: Obj[] | undefined, over: Obj[] | undefined): Obj[] {
  const out: Obj[] = clone(base ?? []);
  for (const entry of over ?? []) {
    const at = out.findIndex((e) => e.name === entry.name);
    if (entry.remove === true) {
      if (at >= 0) out.splice(at, 1);
      continue;
    }
    if (at < 0) out.push(clone(entry));
    else if (entry.component !== undefined && entry.component !== out[at].component) out[at] = clone(entry);
    else out[at] = { ...out[at], ...clone(entry) };
  }
  return out;
}

function kindChain(doc: Obj, kind: string, seen: Set<string> = new Set()): Obj {
  if (seen.has(kind)) throw new Error(`page kind ${kind} extends itself`);
  seen.add(kind);
  const own = doc.page_kinds?.[kind];
  if (own) {
    const base = own.extends ? kindChain(doc, own.extends, seen) : {};
    return mergeKind(base, own);
  }
  const builtin = BUILTIN_KINDS[kind];
  if (!builtin) throw new Error(`unknown page kind ${kind}`);
  return clone(builtin);
}

function mergeKind(base: Obj, over: Obj): Obj {
  return {
    state: mergeMaps(base.state, over.state),
    sections: mergeNamed(base.sections, over.sections),
    overlays: mergeMaps(base.overlays, over.overlays),
    header: mergeMaps(base.header, over.header),
    layout: over.layout ?? base.layout,
  };
}

export function field(f: Json): Field {
  if (typeof f === 'string') return { field: f, name: f };
  return { ...f, name: f.name ?? f.field };
}

export function reads(r: Json): Reads | undefined {
  if (r === undefined || r === null) return undefined;
  if (typeof r === 'string') return { view: r };
  return r;
}

const snake = (s: string) => s.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();

/** The derived name of an action (`Action.name` default). */
export function actionName(a: Obj): string {
  if (a.name) return a.name;
  if (a.opens) return a.opens;
  if (a.does) return snake(String(a.does).split('.').pop()!);
  if (a.navigate?.to) return a.navigate.to;
  if (a.export?.reads) return a.export.reads;
  if (a.upload?.does) return a.upload.does;
  if (a.sets) return Object.keys(a.sets)[0];
  return 'action';
}

function action(a: Obj): Action {
  return { ...a, name: actionName(a) };
}

const FIELD_LISTS = ['columns', 'fields', 'inputs'];
const ACTION_LISTS = ['row_actions', 'bulk_actions', 'actions', 'item_actions', 'alternatives'];

/** Expands the short forms inside one node (section, overlay, nested composite), recursively. */
export function normalizeNode(n: Obj): Obj {
  const out: Obj = { ...n };
  if (typeof out.reads === 'string' || isObj(out.reads)) out.reads = reads(out.reads);
  if (out.loads !== undefined) out.loads = reads(out.loads);
  for (const key of FIELD_LISTS) {
    if (Array.isArray(out[key])) out[key] = out[key].map((f: Json) => normalizeField(f));
  }
  for (const key of ACTION_LISTS) {
    if (Array.isArray(out[key])) out[key] = out[key].map((a: Obj) => action(a));
  }
  if (Array.isArray(out.groups)) {
    out.groups = out.groups.map((g: Obj) => ({ ...g, fields: (g.fields ?? []).map((f: Json) => normalizeField(f)), actions: (g.actions ?? []).map(action) }));
  }
  if (Array.isArray(out.tabs)) out.tabs = out.tabs.map((t: Obj) => ({ ...t, fields: Array.isArray(t.fields) ? t.fields.map(normalizeField) : t.fields }));
  for (const key of ['expand', 'record', 'result']) if (isObj(out[key])) out[key] = normalizeNode(out[key]);
  for (const key of ['item', 'children', 'parts', 'body', 'choices', 'metrics', 'toolbar']) {
    if (Array.isArray(out[key])) out[key] = out[key].map((c: Json) => (typeof c === 'string' ? { component: c, name: c } : normalizeNode(c)));
  }
  if (isObj(out.states?.empty?.action)) out.states = { ...out.states, empty: { ...out.states.empty, action: action(out.states.empty.action) } };
  if (isObj(out.action)) out.action = action(out.action);
  return out;
}

function normalizeField(f: Json): Field {
  const nf = field(f);
  if (isObj(nf.choice)) nf.choice = normalizeNode(nf.choice);
  return nf;
}

/** Merges every page over its kind and expands short forms. Returns a new document. */
export function normalize(raw: Obj): Doc {
  const doc: Obj = clone(raw);
  if (doc.format !== 'ess-ui/1') throw new Error(`not an ess-ui/1 document (format: ${doc.format})`);
  const pages: Record<string, Page> = {};
  for (const [name, page] of Object.entries<Obj>(doc.pages ?? {})) {
    const base = kindChain(doc, page.kind);
    const merged = mergeKind(base, page);
    const header = merged.header ?? {};
    if (header.title === 'from_page' || header.title === undefined) header.title = page.title;
    if (Array.isArray(header.actions)) header.actions = header.actions.map(action);
    const overlays: Record<string, Obj> = {};
    for (const [k, o] of Object.entries<Obj>(merged.overlays ?? {})) overlays[k] = normalizeNode(o);
    pages[name] = {
      ...page,
      name,
      state: merged.state ?? {},
      sections: (merged.sections as Obj[]).map((s) => normalizeNode(s) as Section),
      overlays,
      header,
      layout: merged.layout ?? 'stack',
    } as Page;
  }
  doc.pages = pages;
  for (const shell of Object.values<Obj>(doc.shells ?? {})) {
    for (const region of Object.values<Obj>(shell.regions ?? {})) {
      if (Array.isArray(region.props?.actions)) region.props.actions = region.props.actions.map(action);
    }
    for (const [k, o] of Object.entries<Obj>(shell.overlays ?? {})) shell.overlays[k] = normalizeNode(o);
  }
  return doc as Doc;
}

// ── The model, read from `ess specify compile --format json` ────────────────────────────────

function typeName(t: Json): { type: string; optional: boolean } {
  if (!t) return { type: 'json', optional: false };
  switch (t.kind) {
    case 'optional':
      return { type: typeName(t.of).type, optional: true };
    case 'primitive':
    case 'declared':
      return { type: t.name, optional: false };
    case 'list':
      return { type: `[${typeName(t.of).type}]`, optional: false };
    case 'map':
      return { type: 'map', optional: false };
    default:
      return { type: String(t.kind ?? 'json'), optional: false };
  }
}

/** Every view, command and event name an ess-ui/1 document mentions (`view:`, `does:`, …). */
export function referencedNames(doc: Json): Set<string> {
  const names = new Set<string>();
  const walk = (v: Json) => {
    if (Array.isArray(v)) v.forEach(walk);
    else if (isObj(v)) {
      for (const [k, x] of Object.entries(v)) {
        if (typeof x === 'string' && ['view', 'does', 'reads', 'by_id_from', 'same_as', 'references', 'completes'].includes(k)) names.add(x);
        if (k === 'commands' && Array.isArray(x)) x.forEach((c) => typeof c === 'string' && names.add(c));
        walk(x);
      }
    }
  };
  walk(doc);
  return names;
}

/** The part of the model the renderer needs: every enum, and the views and commands the document names. */
export function modelMeta(ir: Obj, doc: Json): ModelMeta {
  const names = referencedNames(doc);
  const enums: Record<string, string[]> = {};
  for (const [name, t] of Object.entries<Obj>(ir.types ?? {})) {
    if (t.body?.kind === 'enum') enums[name] = t.body.variants.map((v: Json) => (typeof v === 'string' ? v : v.name));
  }
  const commands: ModelMeta['commands'] = {};
  for (const [name, c] of Object.entries<Obj>(ir.commands ?? {})) {
    if (!names.has(name)) continue;
    commands[name] = { inputs: (c.input ?? []).map((i: Obj) => ({ name: i.name, ...typeName(i.type_ref) })) };
  }
  const views: ModelMeta['views'] = {};
  for (const [name, v] of Object.entries<Obj>(ir.views ?? {})) {
    if (!names.has(name)) continue;
    views[name] = { params: (v.params ?? []).map((p: Obj) => p.name), fields: (v.fields ?? []).map((f: Obj) => f.name) };
  }
  return { system: ir.system?.name ?? ir.system ?? '', enums, commands, views };
}

/** An enum by the name a choice gives it: qualified, or the last segments only one enum ends with. */
export function findEnum(model: ModelMeta, doc: Doc, name: string): string[] | undefined {
  const own = doc.types?.[name];
  if (isObj(own) && Array.isArray(own.enum)) return own.enum.map(String);
  if (model.enums[name]) return model.enums[name];
  const hits = Object.keys(model.enums).filter((k) => k.endsWith(`.${name}`));
  return hits.length === 1 ? model.enums[hits[0]] : undefined;
}
