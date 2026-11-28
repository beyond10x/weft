// Starts one console: the theme stylesheet from the document's tokens, the hash router, the shell.

import { StrictMode, useCallback, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Shell } from './renderer/Shell.tsx';
import type { Brand } from './renderer/Shell.tsx';
import type { HostComponent } from './renderer/runtime.tsx';
import { href, parseHash } from './renderer/route.ts';
import { resolveTheme, themeCss } from './renderer/theme.ts';
import type { Bundle, Json } from './renderer/types.ts';
import './styles/app.css';

function Root({ bundle, hosts, brand }: { bundle: Bundle; hosts: Record<string, HostComponent>; brand: Brand }) {
  const doc = bundle.doc;
  const [route, setRoute] = useState(() => parseHash(doc, window.location.hash));
  useEffect(() => {
    const f = () => setRoute(parseHash(doc, window.location.hash));
    window.addEventListener('hashchange', f);
    return () => window.removeEventListener('hashchange', f);
  }, [doc]);
  const go = useCallback((h: string) => { window.location.hash = h.replace(/^#/, ''); }, []);
  const setQuery = useCallback((q: Record<string, Json>) => {
    const h = href(doc, route.page, route.params, q);
    window.history.replaceState(null, '', `${window.location.pathname}${window.location.search}${h}`);
    setRoute(parseHash(doc, h));
  }, [doc, route.page, route.params]);
  return <Shell bundle={bundle} route={route} go={go} setQuery={setQuery} hosts={hosts} brand={brand} />;
}

export function boot(bundle: Bundle, hosts: Record<string, HostComponent>, brand: Brand) {
  const style = document.createElement('style');
  style.dataset.essUi = bundle.doc.app;
  style.textContent = themeCss(bundle.doc);
  document.head.appendChild(style);
  // The first paint already in the chosen theme (the shell keeps it in sync after).
  const themeKey = bundle.doc.theme?.chosen_by?.replace(/^shell\./, '') ?? 'theme';
  let chosen: string | undefined;
  try { chosen = JSON.parse(window.localStorage.getItem(`weft.${bundle.doc.app}.shell.${themeKey}`) ?? 'null') ?? undefined; } catch { chosen = undefined; }
  document.documentElement.dataset.theme = resolveTheme(bundle.doc, chosen, !!window.matchMedia?.('(prefers-color-scheme: dark)').matches);
  document.documentElement.dataset.app = bundle.doc.app;
  const el = document.getElementById('root');
  if (!el) throw new Error('no #root element');
  createRoot(el).render(<StrictMode><Root bundle={bundle} hosts={hosts} brand={brand} /></StrictMode>);
}
