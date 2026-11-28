// The app shell (`shells.app`): navigation from `navigation.sections`, the account menu with the
// shell's actions (theme), the page outlet, the notification region and the confirm dialog. On a
// narrow screen the navigation is a drawer behind a menu button.

import { useEffect, useRef, useState } from 'react';
import { App, useApp, useAppRuntime } from './runtime.tsx';
import type { HostComponent } from './runtime.tsx';
import { href } from './route.ts';
import type { Route } from './route.ts';
import { resolveTheme } from './theme.ts';
import { PageView } from './Page.tsx';
import { Button, EmptyState, Menu, Modal, Toasts } from '../ui/kit.tsx';
import { Icon, hasIcon } from '../ui/Icon.tsx';
import type { Action, Bundle, Json, Obj } from './types.ts';

export interface Brand {
  /** The console's name under the product mark. */
  name: string;
  /** The other console: label and URL. */
  other?: { label: string; url: string };
}

function usePrefersDark(): boolean {
  const q = typeof window !== 'undefined' ? window.matchMedia?.('(prefers-color-scheme: dark)') : undefined;
  const [dark, setDark] = useState(!!q?.matches);
  useEffect(() => {
    if (!q) return;
    const f = () => setDark(q.matches);
    q.addEventListener('change', f);
    return () => q.removeEventListener('change', f);
  }, [q]);
  return dark;
}

function Nav({ route, onNavigate, brand }: { route: Route; onNavigate: () => void; brand: Brand }) {
  const app = useApp();
  const sections: Obj[] = app.doc.navigation.sections ?? [];
  const isOn = (p: string) => route.page === p || route.page.startsWith(`${p}.`);
  return (
    <nav className="nav-links" aria-label="Main">
      {sections.map((s) => (
        <div key={s.name} className="nav-group">
          <p className="nav-group-label">{s.label ?? s.name}</p>
          <ul>
            {(s.pages ?? []).map((p: string) => {
              const page = app.doc.pages[p];
              if (!page) return null;
              return (
                <li key={p}>
                  <a href={href(app.doc, p)} aria-current={isOn(p) ? 'page' : undefined} className="nav-link" onClick={onNavigate}>
                    <Icon name={hasIcon(s.icon) && (s.pages ?? [])[0] === p ? s.icon : navIcon(p, s.icon)} size={17} />
                    <span>{page.nav?.label ?? page.title}</span>
                  </a>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
      {brand.other && (
        <div className="nav-foot">
          <a className="nav-other" href={brand.other.url}><Icon name="logout" size={16} /><span>{brand.other.label}</span></a>
        </div>
      )}
    </nav>
  );
}

const PAGE_ICONS: Record<string, string> = {
  overview: 'gauge', home: 'home', nodes: 'server', node_pools: 'layers', zones: 'plug', join_tokens: 'copy', inventory: 'database',
  scheduling: 'calendar', placements: 'layers', priority_classes: 'chart', tenants: 'users', quotas: 'gauge', platform: 'layers',
  formations: 'layers', workloads: 'server', services: 'plug', instances: 'terminal', releases: 'rocket', catalog: 'database',
  managed: 'database', secrets: 'info', usage: 'chart',
};
function navIcon(page: string, fallback?: string): string {
  return PAGE_ICONS[page] ?? fallback ?? 'dot';
}

function AccountMenu() {
  const app = useApp();
  const region: Obj | undefined = app.doc.shells.app?.regions?.account;
  const actions: Action[] = region?.props?.actions ?? [];
  const display = String(app.actor?.display ?? 'Signed in');
  const initials = display.split(/[^a-zA-Z]+/).filter(Boolean).slice(0, 2).map((w) => w[0]!.toUpperCase()).join('') || '·';
  const items = actions.map((a) => {
    const entry = Object.entries(a.sets ?? {})[0] as [string, Json] | undefined;
    const target = entry?.[0];
    const v = entry?.[1];
    const shellKey = target?.startsWith('shell.') ? target.slice(6) : undefined;
    return {
      key: a.name,
      label: a.label ?? a.name,
      checked: shellKey ? app.shell[shellKey] === v : undefined,
      onSelect: () => { if (shellKey) app.setShell(shellKey, v); },
    };
  });
  return (
    <Menu
      label={`Account: ${display}`}
      items={items}
      text={<><span className="avatar" aria-hidden="true">{initials}</span><span className="account-name">{display}</span><Icon name="chevron_down" size={15} /></>}
    />
  );
}

function Mark() {
  return (
    <svg className="mark" width="26" height="26" viewBox="0 0 26 26" aria-hidden="true">
      <rect width="26" height="26" rx="7" fill="var(--color-accent)" />
      <path d="M6 9l3.2 9L13 9l3.8 9L20 9" fill="none" stroke="var(--color-on_accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function ShellFrame({ brand }: { brand: Brand }) {
  const app = useApp();
  const [navOpen, setNavOpen] = useState(false);
  const main = useRef<HTMLElement>(null);
  const first = useRef(true);
  const routeKey = `${app.route.page}|${JSON.stringify(app.route.params)}`;
  useEffect(() => {
    if (first.current) { first.current = false; return; }
    setNavOpen(false);
    document.getElementById('page-title')?.focus();
    main.current?.scrollTo?.(0, 0);
    window.scrollTo(0, 0);
  }, [routeKey]);
  const page = app.doc.pages[app.route.page];
  return (
    <div className={`shell${navOpen ? ' nav-open' : ''}`}>
      <a className="skip-link" href="#main" onClick={(e) => { e.preventDefault(); document.getElementById('page-title')?.focus(); }}>Skip to content</a>
      <aside className="sidebar" aria-label="Navigation">
        <div className="brand">
          <Mark />
          <div className="brand-text">
            <span className="brand-product">weft</span>
            <span className="brand-console">{brand.name}</span>
          </div>
          <button type="button" className="btn btn-ghost btn-icon nav-close" aria-label="Close navigation" onClick={() => setNavOpen(false)}>
            <Icon name="close" />
          </button>
        </div>
        <Nav route={app.route} onNavigate={() => setNavOpen(false)} brand={brand} />
      </aside>
      <div className="nav-scrim" aria-hidden="true" onClick={() => setNavOpen(false)} />
      <div className="frame">
        <header className="topbar">
          <button type="button" className="btn btn-ghost btn-icon nav-toggle" aria-label="Open navigation" aria-expanded={navOpen} onClick={() => setNavOpen(true)}>
            <Icon name="menu" />
          </button>
          <span className="topbar-brand"><Mark /><span>{brand.name}</span></span>
          <span className="topbar-env"><span className="env-dot" aria-hidden="true" />Fixture data</span>
          <div className="topbar-end"><AccountMenu /></div>
        </header>
        <main id="main" ref={main} className="main">
          {page ? (
            <PageView key={routeKey} page={page} params={app.route.params} query={app.route.query} />
          ) : (
            <div className="page">
              <header className="page-head"><h1 id="page-title" tabIndex={-1}>Page not found</h1></header>
              <EmptyState message={`No page answers ${app.route.params.path ?? 'this address'}.`} action={<a className="btn btn-primary" href={href(app.doc, String(app.doc.navigation.home))}>Go to {app.doc.pages[String(app.doc.navigation.home)]?.title}</a>} />
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

export function Shell({ bundle, route, go, setQuery, hosts, brand }: { bundle: Bundle; route: Route; go: (h: string) => void; setQuery: (q: Record<string, Json>) => void; hosts: Record<string, HostComponent>; brand: Brand }) {
  const { app, toasts, dismiss, confirmState, closeConfirm } = useAppRuntime(bundle, route, go, setQuery, hosts);
  const dark = usePrefersDark();
  const themeKey = bundle.doc.theme?.chosen_by?.replace(/^shell\./, '') ?? 'theme';
  const theme = resolveTheme(bundle.doc, app.shell[themeKey], dark);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.app = bundle.doc.app;
  }, [theme, bundle.doc.app]);
  return (
    <App.Provider value={app}>
      <ShellFrame brand={brand} />
      <Toasts toasts={toasts} onDismiss={dismiss} />
      {confirmState && (
        <Modal
          kind="dialog"
          title={confirmState.title}
          onClose={() => closeConfirm(false)}
          footer={<>
            <Button kind="ghost" onClick={() => closeConfirm(false)}>Cancel</Button>
            <Button kind={confirmState.danger ? 'danger' : 'primary'} onClick={() => closeConfirm(true)}>{confirmState.confirmLabel ?? 'Confirm'}</Button>
          </>}
        >
          <p>{confirmState.body ?? 'This cannot be undone from here.'}</p>
        </Modal>
      )}
    </App.Provider>
  );
}
