// One page: its state (URL-backed where the document says `store: url`), its header (title,
// title_from, badge, total, actions, switch, help, filters), its sections in the declared layout
// (stack, columns, areas), and its overlays (drawer, dialog).

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { PageContext, SectionStore, useApp, useSectionResult } from './runtime.tsx';
import type { PageCtx } from './runtime.tsx';
import type { Scope } from './expr.ts';
import { value } from './expr.ts';
import { href } from './route.ts';
import { toneOf, words } from './format.ts';
import { NodeView } from './nodes/NodeView.tsx';
import type { RowsReport } from './nodes/Collection.tsx';
import { windowValue } from './nodes/FilterBar.tsx';
import { ActionBar } from './nodes/actions.tsx';
import { Badge, Card, EmptyState, Modal } from '../ui/kit.tsx';
import type { Json, Obj, Page, Section } from './types.ts';

function parseQuery(v: string | undefined): Json {
  if (v === undefined) return undefined;
  if (/^[[{]/.test(v)) {
    try { return JSON.parse(v); } catch { return v; }
  }
  if (/^-?\d+(\.\d+)?$/.test(v)) return Number(v);
  return v;
}

function SectionFrame({ section, scope, store }: { section: Section; scope: Scope; store: SectionStore }) {
  const report = useCallback((r: RowsReport) => store.set(section.name, { status: r.status, rows: r.rows, total: r.total }), [store, section.name]);
  const res = useSectionResult(store, section.name);
  const title = section.title;
  const comp = section.host ? 'host' : section.component;
  if (comp === 'filter_bar') return <div className="section-bar"><NodeView node={section} scope={scope} /></div>;
  if (comp === 'metric') return <div className="card card-metric"><NodeView node={section} scope={scope} report={report} /></div>;
  const count = comp === 'collection' && res && (res.status === 'ready') ? res.total : undefined;
  return (
    <Card title={title} count={count} busy={res?.status === 'loading'} className={`card-${comp}`}>
      <NodeView node={section} scope={scope} title={title} report={report} />
    </Card>
  );
}

function Header({ page, scope, store, sectionFor }: { page: Page; scope: Scope; store: SectionStore; sectionFor: (name: string) => ReactNode }) {
  const app = useApp();
  const h = page.header ?? {};
  const from = useSectionResult(store, h.title_from?.section);
  const totalOf = useSectionResult(store, typeof h.total === 'string' ? h.total : undefined);
  useSectionResult(store, h.badge?.text?.match(/^section\.([a-z0-9_]+)\./)?.[1]);
  const fromTitle = h.title_from ? from?.rows?.[0]?.[h.title_from.field] : undefined;
  const title = String(fromTitle ?? h.title ?? page.title ?? words(page.name));
  const badgeText = h.badge ? value(h.badge.text, scope) : undefined;
  const badgeTone = h.badge?.tone_by ? toneOf(badgeText, app.doc.tone_maps?.[h.badge.tone_by], 'neutral') : toneOf(badgeText, undefined, h.badge?.tone ?? 'neutral');
  const crumbParent = page.name.includes('.') ? page.name.split('.')[0] : undefined;
  const parent = crumbParent && app.doc.pages[crumbParent] ? app.doc.pages[crumbParent] : undefined;
  const sw: string[] = Array.isArray(h.switch) ? h.switch : [];
  const h1 = useRef<HTMLHeadingElement>(null);
  useEffect(() => { document.title = `${title} · ${app.doc.title ?? app.doc.app}`; }, [title, app.doc]);
  return (
    <header className="page-head">
      {parent && (
        <nav aria-label="Breadcrumb" className="crumbs">
          <a href={href(app.doc, parent.name)}>{parent.nav?.label ?? parent.title}</a>
          <span aria-hidden="true">/</span>
          <span aria-current="page">{fromTitle ? title : page.title}</span>
        </nav>
      )}
      <div className="page-title-row">
        <div className="page-title-block">
          <h1 ref={h1} tabIndex={-1} id="page-title">
            {title}
            {totalOf?.status === 'ready' && <span className="count" aria-label={`${totalOf.total} rows`}>{totalOf.total}</span>}
          </h1>
          {badgeText !== undefined && badgeText !== null && badgeText !== '' && <Badge tone={badgeTone}>{words(String(badgeText))}</Badge>}
        </div>
        <div className="page-actions"><ActionBar actions={h.actions} scope={scope} primary handed={scope.params} /></div>
      </div>
      {h.help?.text && <p className="page-help">{h.help.text}</p>}
      {sw.length > 0 && (
        <nav className="tabs" aria-label={`${page.title} views`}>
          {sw.map((p) => {
            const target = app.doc.pages[p];
            if (!target) return null;
            const params = Object.fromEntries(Object.keys(target.params ?? {}).map((k) => [k, scope.params?.[k]]));
            return (
              <a key={p} href={href(app.doc, p, params)} className={`tab${p === page.name ? ' tab-on' : ''}`} aria-current={p === page.name ? 'page' : undefined}>
                {target.nav?.label ?? target.title}
              </a>
            );
          })}
        </nav>
      )}
      {headerFilters(page) && sectionFor(headerFilters(page)!)}
    </header>
  );
}

/** The section the header draws as its filters: `header.filters`, unless the layout places it. */
export function headerFilters(page: Page): string | undefined {
  const f = typeof page.header?.filters === 'string' ? page.header.filters : undefined;
  const l = page.layout;
  if (!f || !l || typeof l !== 'object') return f;
  if (Array.isArray(l.columns) && l.columns.some((c: Obj) => c.sections?.includes(f))) return undefined;
  if (l.areas && Object.values<string[]>(l.areas.place ?? {}).some((ns) => ns.includes(f))) return undefined;
  return f;
}

function Layout({ page, render }: { page: Page; render: (name: string) => ReactNode }) {
  const placedInHeader = headerFilters(page);
  const all = page.sections.map((s) => s.name);
  const l = page.layout;
  if (l && typeof l === 'object' && Array.isArray(l.columns)) {
    const used = new Set(l.columns.flatMap((c: Obj) => c.sections));
    const rest = all.filter((n) => !used.has(n) && n !== placedInHeader);
    return (
      <>
        <div className="layout-columns">
          {l.columns.map((c: Obj) => (
            <div key={c.name} className={`layout-col col-${c.width ?? 'auto'}`}>{c.sections.map((n: string) => <Slot key={n}>{render(n)}</Slot>)}</div>
          ))}
        </div>
        {rest.length > 0 && <div className="layout-stack">{rest.map((n) => <Slot key={n}>{render(n)}</Slot>)}</div>}
      </>
    );
  }
  if (l && typeof l === 'object' && l.areas) {
    const grid: string[][] = l.areas.grid ?? [];
    const place: Record<string, string[]> = l.areas.place ?? {};
    const used = new Set(Object.values(place).flat());
    const rest = all.filter((n) => !used.has(n) && n !== placedInHeader);
    return (
      <div className="layout-areas">
        {grid.map((row, i) => {
          const cells: { area: string; span: number }[] = [];
          for (const a of row) {
            const last = cells[cells.length - 1];
            if (last && last.area === a) last.span += 1;
            else cells.push({ area: a, span: 1 });
          }
          const names = cells.map((c) => place[c.area] ?? []);
          if (names.every((ns) => ns.length === 0)) return null;
          return (
            <div key={i} className="area-row" style={{ '--cols': row.length } as CSSProperties}>
              {cells.map((c, j) => (
                <div key={c.area} className={`area-cell${c.span > 1 ? ' area-wide' : ''}${c.span === row.length ? ' area-full' : ''}`} style={{ '--span': c.span } as CSSProperties}>
                  {names[j].map((n) => <Slot key={n}>{render(n)}</Slot>)}
                </div>
              ))}
            </div>
          );
        })}
        {rest.map((n) => <Slot key={n}>{render(n)}</Slot>)}
      </div>
    );
  }
  return <div className="layout-stack">{all.filter((n) => n !== placedInHeader).map((n) => <Slot key={n}>{render(n)}</Slot>)}</div>;
}

function Slot({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

export function PageView({ page, params, query }: { page: Page; params: Record<string, string>; query: Record<string, string> }) {
  const app = useApp();
  const store = useMemo(() => new SectionStore(), []);
  const decls = page.state ?? {};
  const [local, setLocal] = useState<Record<string, Json>>(() => {
    const out: Record<string, Json> = {};
    for (const [k, d] of Object.entries<Obj>(decls)) if (d.store !== 'url') out[k] = d.default;
    return out;
  });
  const state = useMemo(() => {
    const out: Record<string, Json> = { ...local };
    for (const [k, d] of Object.entries<Obj>(decls)) {
      if (d.store === 'url') {
        const q = parseQuery(query[k]);
        out[k] = q !== undefined ? q : d.default;
      }
      out[k] = windowValue(out[k]);
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [local, JSON.stringify(query), page.name]);
  const urlNames = Object.entries<Obj>(decls).filter(([, d]) => d.store === 'url').map(([k]) => k);
  const setState = useCallback((name: string, v: Json) => {
    if (decls[name]?.store === 'url') {
      const q: Record<string, Json> = {};
      for (const k of urlNames) if (query[k] !== undefined) q[k] = query[k];
      const d = decls[name]?.default;
      if (v === undefined || v === null || v === '' || JSON.stringify(v) === JSON.stringify(d)) delete q[name];
      else q[name] = typeof v === 'object' ? JSON.stringify(v.preset && v.preset !== 'custom' ? { preset: v.preset } : v) : v;
      app.setQuery(q);
    } else {
      setLocal((s) => ({ ...s, [name]: v }));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(query), page.name, app]);

  const [overlay, setOverlay] = useState<{ name: string; handed: Record<string, Json> } | null>(null);
  const openOverlay = useCallback((name: string, handed: Record<string, Json>) => setOverlay({ name, handed }), []);

  const scope: Scope = useMemo(() => ({
    state,
    params,
    actor: app.actor,
    shell: app.shell,
    url: query,
    section: (n: string) => {
      const r = store.get(n);
      return r ? { row: r.rows[0], selection: r.selection } : undefined;
    },
    may: () => true,
  }), [state, params, app.actor, app.shell, query, store]);

  const ctx: PageCtx = { page, params, state, setState, sections: store, openOverlay, scope };
  const byName = new Map(page.sections.map((s) => [s.name, s]));
  const render = (name: string) => {
    const s = byName.get(name);
    return s ? <SectionFrame key={name} section={s} scope={scope} store={store} /> : null;
  };
  const o = overlay ? page.overlays?.[overlay.name] : undefined;
  const oScope: Scope | undefined = overlay ? { ...scope, params: { ...params, ...overlay.handed }, args: overlay.handed, row: undefined } : undefined;
  return (
    <PageContext.Provider value={ctx}>
      <div className={`page page-${page.kind}`}>
        <Header page={page} scope={scope} store={store} sectionFor={render} />
        <Layout page={page} render={render} />
      </div>
      {overlay && o && oScope && (
        <Modal kind={o.kind ?? 'drawer'} title={o.title ?? words(overlay.name)} onClose={() => setOverlay(null)}>
          {o.component === 'form' ? (
            <NodeView node={o} scope={oScope} onDone={() => setOverlay(null)} />
          ) : (
            <NodeView node={o} scope={oScope} title={o.title} />
          )}
        </Modal>
      )}
      {overlay && !o && (
        <Modal kind="dialog" title="Not found" onClose={() => setOverlay(null)}>
          <EmptyState message={`This page declares no overlay ${overlay.name}.`} />
        </Modal>
      )}
    </PageContext.Provider>
  );
}
