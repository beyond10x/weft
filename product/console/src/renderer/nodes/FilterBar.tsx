// `filter_bar`: a search box, choices (a dropdown, or segmented buttons for `style: segmented`) and a
// time window with presets, each bound to page state (`binds: state.<name>`).

import { useId } from 'react';
import { useApp, usePage, useRead } from '../runtime.tsx';
import type { Scope } from '../expr.ts';
import { findEnum } from '../load.ts';
import { plain, words } from '../format.ts';
import { Icon } from '../../ui/Icon.tsx';
import type { Json, Obj } from '../types.ts';

const PRESETS: { key: string; label: string }[] = [
  { key: 'this_month', label: 'This month' },
  { key: 'last_30d', label: 'Last 30 days' },
  { key: 'this_quarter', label: 'This quarter' },
];

/** The from/to of a window preset, in UTC, as of `now`. */
export function windowRange(preset: string, now: Date = new Date()): { from: string; to: string } {
  const to = now.toISOString();
  const y = now.getUTCFullYear();
  const m = now.getUTCMonth();
  if (preset === 'this_month') return { from: new Date(Date.UTC(y, m, 1)).toISOString(), to };
  if (preset === 'this_quarter') return { from: new Date(Date.UTC(y, m - (m % 3), 1)).toISOString(), to };
  return { from: new Date(now.getTime() - 30 * 86_400_000).toISOString(), to };
}

/** A window state value with its from/to filled in from its preset. */
export function windowValue(v: Json): Json {
  if (!v || typeof v !== 'object' || !v.preset || v.preset === 'custom' || v.from) return v;
  return { ...v, ...windowRange(v.preset) };
}

const stateName = (binds: string | undefined) => (binds?.startsWith('state.') ? binds.slice(6) : undefined);

export interface ChoiceOption {
  value: string;
  label: string;
}

/** The options of a `choice` node: the variants of a model enum, a list, or the rows of a read. */
export function useChoiceOptions(choice: Obj | undefined, scope: Scope): { options: ChoiceOption[]; status: string } {
  const app = useApp();
  const read = useRead(choice?.reads, scope);
  if (!choice) return { options: [], status: 'ready' };
  if (Array.isArray(choice.options)) return { options: choice.options.map((o: Json) => (typeof o === 'object' ? { value: String(o.value), label: String(o.label ?? o.value) } : { value: String(o), label: words(String(o)) })), status: 'ready' };
  if (typeof choice.options === 'string') {
    const vs = findEnum(app.bundle.model, app.doc, choice.options) ?? [];
    return { options: vs.map((v) => ({ value: v, label: words(v) })), status: 'ready' };
  }
  const opts = read.rows.map((r) => ({ value: String(r?.[choice.value ?? 'id']), label: plain(r?.[choice.label ?? choice.value ?? 'name']) }));
  return { options: opts, status: read.status };
}

function Choice({ node, scope }: { node: Obj; scope: Scope }) {
  const page = usePage();
  const id = useId();
  const name = stateName(node.binds);
  const { options, status } = useChoiceOptions(node, scope);
  const current = name ? page.state[name] : undefined;
  const label = node.label ?? words(node.name);
  if (node.style === 'segmented') {
    return (
      <fieldset className="segmented">
        <legend className="sr-only">{label}</legend>
        {options.map((o) => (
          <label key={o.value} className={`seg${current === o.value ? ' seg-on' : ''}`}>
            <input type="radio" name={id} value={o.value} checked={current === o.value} onChange={() => name && page.setState(name, o.value)} />
            <span>{o.label}</span>
          </label>
        ))}
      </fieldset>
    );
  }
  return (
    <label className="filter-choice">
      <span className="filter-label">{label}</span>
      <select value={current ?? ''} onChange={(e) => name && page.setState(name, e.target.value || null)} disabled={status === 'idle'} aria-busy={status === 'loading' || undefined}>
        <option value="">{status === 'idle' ? 'Pick the one above first' : status === 'loading' ? 'Loading…' : node.placeholder ?? 'All'}</option>
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
    </label>
  );
}

export function FilterBar({ node, scope }: { node: Obj; scope: Scope }) {
  const page = usePage();
  const search = node.search;
  const searchName = stateName(search?.binds);
  const win = node.window;
  const winName = stateName(win?.binds);
  const winVal = winName ? page.state[winName] : undefined;
  const id = useId();
  return (
    <div className="filter-bar" role="search" aria-label={node.label ?? 'Filters'}>
      {searchName && (
        <label className="filter-search">
          <Icon name="search" size={16} />
          <span className="sr-only">{search.label ?? 'Search'}</span>
          <input
            type="search"
            value={String(page.state[searchName] ?? '')}
            placeholder={search.placeholder ?? 'Search'}
            onChange={(e) => page.setState(searchName, e.target.value)}
          />
        </label>
      )}
      {(node.choices ?? []).map((c: Obj) => <Choice key={c.name} node={c} scope={scope} />)}
      {winName && (
        <fieldset className="segmented">
          <legend className="sr-only">Time window</legend>
          {PRESETS.map((p) => (
            <label key={p.key} className={`seg${winVal?.preset === p.key ? ' seg-on' : ''}`}>
              <input type="radio" name={id} value={p.key} checked={winVal?.preset === p.key} onChange={() => page.setState(winName, { preset: p.key, ...windowRange(p.key) })} />
              <span>{p.label}</span>
            </label>
          ))}
        </fieldset>
      )}
    </div>
  );
}
