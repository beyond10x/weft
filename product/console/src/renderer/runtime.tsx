// The runtime of the renderer: the app context (document, fixtures, actor, route, shell state,
// notifications, commands), the page context (page state, section results, overlays) and the read
// hook every section, choice and label uses.

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import type { ComponentType } from 'react';
import { FixtureServer, NoFixture } from './fixtures.ts';
import type { ReadResult } from './fixtures.ts';
import { values } from './expr.ts';
import type { Scope } from './expr.ts';
import type { Bundle, Doc, Json, Obj, Page, Reads, Tone } from './types.ts';
import type { Route } from './route.ts';

export interface HostProps {
  /** The slot node as the document writes it (`host`, `reason`, `reads`, `commands`, field keys). */
  slot: Obj;
  /** The row of a field-scope slot; the rows of a section-scope slot's `reads`. */
  row?: Json;
  rows?: Json[];
  value?: Json;
  label?: string;
  /** Host data of fixture mode (`fixtures/<app>/hosts/<Host>.yaml`). */
  data?: Json;
}
export type HostComponent = ComponentType<HostProps>;

export interface ConfirmOptions {
  title: string;
  body?: string;
  confirmLabel?: string;
  danger?: boolean;
}

export interface CommandRecord {
  command: string;
  input: Record<string, Json>;
  at: string;
}

export interface AppCtx {
  bundle: Bundle;
  doc: Doc;
  server: FixtureServer;
  actor: Obj;
  route: Route;
  go: (href: string) => void;
  setQuery: (q: Record<string, Json>) => void;
  shell: Record<string, Json>;
  setShell: (name: string, v: Json) => void;
  notify: (tone: Tone, title: string, body?: string) => void;
  runCommand: (command: string, input: Record<string, Json>) => void;
  confirm: (o: ConfirmOptions) => Promise<boolean>;
  hosts: Record<string, HostComponent>;
  readView: (view: string, params: Record<string, Json>) => Promise<ReadResult>;
  peekView: (view: string, params: Record<string, Json>) => ReadResult | undefined;
}

export const App = createContext<AppCtx | null>(null);
export function useApp(): AppCtx {
  const a = useContext(App);
  if (!a) throw new Error('no app context');
  return a;
}

// ── Section results: what `header.total`, `title_from` and `section.<name>.row` read ────────

export interface SectionResult {
  status: ReadStatus;
  rows: Json[];
  total: number;
  selection?: Json;
}

export class SectionStore {
  private map = new Map<string, SectionResult>();
  private subs = new Set<() => void>();
  get(name: string): SectionResult | undefined { return this.map.get(name); }
  set(name: string, r: SectionResult) {
    const before = this.map.get(name);
    if (before && before.status === r.status && before.rows === r.rows && before.total === r.total && before.selection === r.selection) return;
    this.map.set(name, r);
    this.subs.forEach((f) => f());
  }
  subscribe = (f: () => void) => { this.subs.add(f); return () => { this.subs.delete(f); }; };
  version = 0;
}

export function useSectionResult(store: SectionStore, name: string | undefined): SectionResult | undefined {
  return useSyncExternalStore(store.subscribe, () => (name ? store.get(name) : undefined));
}

export interface PageCtx {
  page: Page;
  params: Record<string, string>;
  state: Record<string, Json>;
  setState: (name: string, v: Json) => void;
  sections: SectionStore;
  openOverlay: (name: string, handed: Record<string, Json>) => void;
  /** The scope expressions on this page evaluate in (plus row, draft, args where they apply). */
  scope: Scope;
}

export const PageContext = createContext<PageCtx | null>(null);
export function usePage(): PageCtx {
  const p = useContext(PageContext);
  if (!p) throw new Error('no page context');
  return p;
}

// ── Reads ──────────────────────────────────────────────────────────────────────────────────

export type ReadStatus = 'idle' | 'loading' | 'ready' | 'empty' | 'refreshing' | 'failed' | 'forbidden';

export interface ReadState {
  status: ReadStatus;
  rows: Json[];
  total: number;
  error?: string;
  retry: () => void;
  view?: string;
  params: Record<string, Json>;
}

const missing = (v: Json) => v === undefined || v === null || v === '';

/** The view and params of a read in a scope; `params` undefined when a bound param has no value yet. */
export function resolveRead(r: Reads | undefined, scope: Scope): { view?: string; params?: Record<string, Json> } {
  if (!r) return {};
  const view = r.view ?? r.placeholder;
  const params = values(r.params, scope);
  for (const v of Object.values(params)) if (missing(v)) return { view };
  return { view, params };
}

/** Reads a view for a section or a nested node, following the section lifecycle of ess-ui/1. */
export function useRead(r: Reads | undefined, scope: Scope): ReadState {
  const app = useApp();
  const { view, params } = resolveRead(r, scope);
  const key = view && params ? `${view}|${JSON.stringify(params)}` : '';
  const [attempt, setAttempt] = useState(0);
  const peek = view && params ? app.peekView(view, params) : undefined;
  const [st, setSt] = useState<{ key: string; status: ReadStatus; rows: Json[]; total: number; error?: string }>(() =>
    peek ? { key, status: peek.rows.length ? 'ready' : 'empty', rows: peek.rows, total: peek.total } : { key, status: key ? 'loading' : 'idle', rows: [], total: 0 },
  );
  useEffect(() => {
    if (!key || !view || !params) {
      setSt({ key, status: 'idle', rows: [], total: 0 });
      return;
    }
    let live = true;
    setSt((s) => (s.key === key && (s.status === 'ready' || s.status === 'empty') ? s : { key, status: s.rows.length && s.key !== key ? 'refreshing' : 'loading', rows: s.rows, total: s.total }));
    app.readView(view, params).then(
      (res) => live && setSt({ key, status: res.rows.length ? 'ready' : 'empty', rows: res.rows, total: res.total }),
      (err) => live && setSt({ key, status: 'failed', rows: [], total: 0, error: err instanceof NoFixture ? `No fixture answers the view ${err.view}.` : String(err?.message ?? err) }),
    );
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, attempt]);
  const retry = useCallback(() => setAttempt((a) => a + 1), []);
  const cur = st.key === key ? st : { status: (key ? 'loading' : 'idle') as ReadStatus, rows: [], total: 0, error: undefined };
  return { status: cur.status, rows: cur.rows, total: cur.total, error: cur.error, retry, view, params: params ?? {} };
}

/** `label_from`: the rows of a view read once, without params, as a map key → label. */
export function useLabelMap(spec: { view: string; field: string; key?: string } | undefined): Map<string, string> | undefined {
  const app = useApp();
  const [map, setMap] = useState<Map<string, string> | undefined>(() => (spec ? toMap(app.peekView(spec.view, {})?.rows, spec) : undefined));
  useEffect(() => {
    if (!spec) return;
    let live = true;
    app.readView(spec.view, {}).then((r) => live && setMap(toMap(r.rows, spec)), () => undefined);
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [spec?.view, spec?.field, spec?.key]);
  return map;
}

function toMap(rows: Json[] | undefined, spec: { field: string; key?: string }): Map<string, string> | undefined {
  if (!rows) return undefined;
  const key = spec.key ?? 'id';
  return new Map(rows.map((r) => [String(r?.[key]), String(r?.[spec.field] ?? '')]));
}

// ── The app's own state: reads with a cache and a latency, commands, notifications ─────────

export function useAppRuntime(bundle: Bundle, route: Route, go: (h: string) => void, setQuery: (q: Record<string, Json>) => void, hosts: Record<string, HostComponent>) {
  const server = useMemo(() => new FixtureServer(bundle.fixtures), [bundle]);
  const cache = useRef(new Map<string, ReadResult>());
  const pending = useRef(new Map<string, Promise<ReadResult>>());
  const latency = useMemo(() => {
    const q = new URLSearchParams(window.location.search).get('latency');
    return q === null ? 160 : Number(q);
  }, []);
  const readView = useCallback((view: string, params: Record<string, Json>) => {
    const key = `${view}|${JSON.stringify(params)}`;
    const hit = cache.current.get(key);
    if (hit) return Promise.resolve(hit);
    const p = pending.current.get(key);
    if (p) return p;
    const next = new Promise<ReadResult>((resolve, reject) => {
      setTimeout(() => {
        pending.current.delete(key);
        try {
          const r = server.read(view, params);
          cache.current.set(key, r);
          resolve(r);
        } catch (e) {
          reject(e);
        }
      }, latency);
    });
    pending.current.set(key, next);
    return next;
  }, [server, latency]);
  const peekView = useCallback((view: string, params: Record<string, Json>) => cache.current.get(`${view}|${JSON.stringify(params)}`), []);

  const shellDecl = bundle.doc.shells.app?.state ?? {};
  const storeKey = (n: string) => `weft.${bundle.doc.app}.shell.${n}`;
  const [shell, setShellState] = useState<Record<string, Json>>(() => {
    const out: Record<string, Json> = {};
    for (const [n, d] of Object.entries<Obj>(shellDecl)) {
      const stored = d.store === 'local_storage' ? window.localStorage.getItem(storeKey(n)) : null;
      out[n] = stored !== null ? JSON.parse(stored) : d.default;
    }
    return out;
  });
  const setShell = useCallback((name: string, v: Json) => {
    setShellState((s) => ({ ...s, [name]: v }));
    if (shellDecl[name]?.store === 'local_storage') window.localStorage.setItem(storeKey(name), JSON.stringify(v));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const [toasts, setToasts] = useState<{ id: number; tone: Tone; title: string; body?: string }[]>([]);
  const nextId = useRef(1);
  const notify = useCallback((tone: Tone, title: string, body?: string) => {
    const id = nextId.current++;
    setToasts((t) => [...t.slice(-3), { id, tone, title, body }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 6000);
  }, []);
  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const runCommand = useCallback((command: string, input: Record<string, Json>) => {
    const rec: CommandRecord = { command, input, at: new Date().toISOString() };
    const w = window as unknown as { __weftCommands?: CommandRecord[] };
    (w.__weftCommands ??= []).push(rec);
    const shown = Object.entries(input).map(([k, v]) => `${k}: ${typeof v === 'object' ? JSON.stringify(v) : String(v)}`).join(', ');
    notify('success', `${command.split('.').pop()} sent`, `Fixture mode, no backend: ${command}${shown ? ` { ${shown} }` : ''}`);
  }, [notify]);

  const [confirmState, setConfirm] = useState<(ConfirmOptions & { resolve: (ok: boolean) => void }) | null>(null);
  const confirm = useCallback((o: ConfirmOptions) => new Promise<boolean>((resolve) => setConfirm({ ...o, resolve })), []);

  const app: AppCtx = {
    bundle, doc: bundle.doc, server, actor: bundle.actor, route, go, setQuery, shell, setShell, notify, runCommand, confirm, hosts, readView, peekView,
  };
  return { app, toasts, dismiss, confirmState, closeConfirm: (ok: boolean) => { confirmState?.resolve(ok); setConfirm(null); } };
}
