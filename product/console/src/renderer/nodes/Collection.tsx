// `collection`: the rows of a view as a dense table (default), cards, a list or a tree; client sort,
// a row filter, groups (`group_by`, `group_order`), an inline `expand`, client paging, and row
// actions (the row's `navigate` action is its link, the rest a menu).

import { Fragment, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { useApp } from '../runtime.tsx';
import { useRead } from '../runtime.tsx';
import type { ReadState } from '../runtime.tsx';
import { holds, values } from '../expr.ts';
import type { Scope } from '../expr.ts';
import { href } from '../route.ts';
import { plain, words } from '../format.ts';
import { FieldValue, toneFor, useLabelMaps } from './Value.tsx';
import type { LabelMaps } from './Value.tsx';
import { ActionBar, RowMenu, visibleActions } from './actions.tsx';
import { ReadStates } from './states.tsx';
import { NodeView } from './NodeView.tsx';
import { Badge, EmptyState, ScrollRegion } from '../../ui/kit.tsx';
import { Icon } from '../../ui/Icon.tsx';
import type { Action, Field, Json, Obj } from '../types.ts';

export interface RowsReport {
  status: ReadState['status'];
  rows: Json[];
  total: number;
}

function fieldLabel(f: Field): string {
  if (f.label) return f.label;
  const s = f.field.replace(/_id$/, '').replace(/_/g, ' ');
  return s.charAt(0).toUpperCase() + s.slice(1);
}
export { fieldLabel };

function compare(a: Json, b: Json): number {
  const x = a ?? '';
  const y = b ?? '';
  if (typeof x === 'number' && typeof y === 'number') return x - y;
  return plain(x).localeCompare(plain(y), 'en', { numeric: true });
}

function rowKey(node: Obj): string {
  return node.reads?.key ?? node.live?.match ?? 'id';
}

/** The row's link: its first `navigate` action (a row of a list opens its record). */
function openAction(actions: Action[] | undefined): Action | undefined {
  return actions?.find((a) => a.navigate && (a.name === 'open' || !a.does));
}

export function Collection({ node, scope, title, report }: { node: Obj; scope: Scope; title?: string; report?: (r: RowsReport) => void }) {
  const read = useRead(node.reads, scope);
  const columns: Field[] = Array.isArray(node.columns) ? node.columns : [];
  const shownCols = columns.filter((c) => !c.visible || c.visible.includes('row.') || holds(c.visible, scope));
  const labels = useLabelMaps(columns, scope.params);
  const filtered = useMemo(
    () => (node.reads?.filter ? read.rows.filter((row) => holds(node.reads.filter, { ...scope, row })) : read.rows),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [read.rows, node.reads?.filter, JSON.stringify(scope.state), JSON.stringify(scope.params)],
  );
  const [sort, setSort] = useState<{ by?: string; dir: 'asc' | 'desc' }>({ by: node.sort?.by, dir: node.sort?.dir ?? 'asc' });
  const sorted = useMemo(() => {
    if (!sort.by) return filtered;
    const by = sort.by;
    const out = [...filtered].sort((a, b) => compare(a?.[by], b?.[by]));
    return sort.dir === 'desc' ? out.reverse() : out;
  }, [filtered, sort]);
  const sizes: number[] | undefined = node.reads?.paging === 'client' ? node.reads?.pager?.sizes ?? [25, 50, 100] : undefined;
  const [size, setSize] = useState(sizes?.[0] ?? 0);
  const [pageIx, setPageIx] = useState(0);
  useEffect(() => setPageIx(0), [filtered.length, size]);
  const pageRows = sizes ? sorted.slice(pageIx * size, pageIx * size + size) : sorted;

  useEffect(() => {
    report?.({ status: read.status, rows: filtered, total: filtered.length });
  }, [read.status, filtered, report]);

  const filteredAway = read.status === 'ready' && filtered.length === 0;
  // A filter that reads no page state (a row of this node) is the section's own: its empty state, not "No match".
  const bySearch = /\bstate\./.test(node.reads?.filter ?? '');
  return (
    <ReadStates read={read} states={node.states} scope={scope}>
      {() => (
        <>
          {filteredAway ? (
            bySearch ? <EmptyState title="No match" message="No row matches the search or filters." /> : <EmptyState title={node.states?.empty?.title} message={node.states?.empty?.message ?? 'Nothing to show yet'} />
          ) : (
            <Rows node={node} scope={scope} rows={pageRows} columns={shownCols} labels={labels} title={title} sort={sort} setSort={setSort} />
          )}
          {sizes && filtered.length > 0 && (
            <Pager total={filtered.length} size={size} sizes={sizes} pageIx={pageIx} setPageIx={setPageIx} setSize={setSize} />
          )}
          {Array.isArray(node.actions) && node.actions.length > 0 && (
            <div className="collection-actions"><ActionBar actions={node.actions} scope={{ ...scope, row: undefined }} size="sm" /></div>
          )}
        </>
      )}
    </ReadStates>
  );
}

function Pager({ total, size, sizes, pageIx, setPageIx, setSize }: { total: number; size: number; sizes: number[]; pageIx: number; setPageIx: (n: number) => void; setSize: (n: number) => void }) {
  const pages = Math.max(1, Math.ceil(total / size));
  const from = total ? pageIx * size + 1 : 0;
  const to = Math.min(total, (pageIx + 1) * size);
  return (
    <nav className="pager" aria-label="Pages">
      <span className="pager-range">{from}–{to} of {total}</span>
      <label className="pager-size">
        <span>Per page</span>
        <select value={size} onChange={(e) => setSize(Number(e.target.value))}>
          {sizes.map((s) => <option key={s} value={s}>{s}</option>)}
        </select>
      </label>
      <button type="button" className="btn btn-ghost btn-icon" aria-label="Previous page" disabled={pageIx === 0} onClick={() => setPageIx(pageIx - 1)}><Icon name="chevron_left" /></button>
      <span className="pager-page">{pageIx + 1} / {pages}</span>
      <button type="button" className="btn btn-ghost btn-icon" aria-label="Next page" disabled={pageIx >= pages - 1} onClick={() => setPageIx(pageIx + 1)}><Icon name="chevron_right" /></button>
    </nav>
  );
}

interface RowsProps {
  node: Obj;
  scope: Scope;
  rows: Json[];
  columns: Field[];
  labels: LabelMaps;
  title?: string;
  sort: { by?: string; dir: 'asc' | 'desc' };
  setSort: (s: { by?: string; dir: 'asc' | 'desc' }) => void;
}

function rowName(row: Json, columns: Field[], key: string): string {
  const first = columns[0];
  const v = first ? row?.[first.field] : row?.[key];
  return plain(v ?? row?.[key] ?? 'row');
}

function Rows(p: RowsProps) {
  const style = p.node.style ?? 'table';
  if (style === 'cards') return <Cards {...p} />;
  if (style === 'list') return <List {...p} />;
  return <Table {...p} tree={style === 'tree'} />;
}

function useRowLink(node: Obj, scope: Scope) {
  const app = useApp();
  const open = openAction(node.row_actions);
  return (row: Json): string | undefined => {
    if (!open?.navigate) return undefined;
    const s = { ...scope, row };
    if (!visibleActions([open], s).length) return undefined;
    return href(app.doc, open.navigate.to, values(open.navigate.params, s));
  };
}

function cellValue(f: Field, row: Json, labels: LabelMaps, scope: Scope, noCopy?: boolean): ReactNode {
  if (f.visible && f.visible.includes('row.') && !holds(f.visible, { ...scope, row })) return null;
  return <FieldValue f={f} row={row} labels={labels} noCopy={noCopy} />;
}

/** Orders tree rows parent first: the parent is the row field `parent_<key>` (a convention: ess-ui/1 names no parent field). */
function treeOrder(rows: Json[], key: string): { row: Json; depth: number }[] {
  const parentField = `parent_${key}`;
  const byParent = new Map<string, Json[]>();
  const ids = new Set(rows.map((r) => String(r?.[key])));
  for (const r of rows) {
    const p = r?.[parentField];
    const k = p && ids.has(String(p)) ? String(p) : '';
    byParent.set(k, [...(byParent.get(k) ?? []), r]);
  }
  const out: { row: Json; depth: number }[] = [];
  const walk = (k: string, depth: number) => {
    for (const r of byParent.get(k) ?? []) {
      out.push({ row: r, depth });
      walk(String(r?.[key]), depth + 1);
    }
  };
  walk('', 0);
  return out.length === rows.length ? out : rows.map((row) => ({ row, depth: 0 }));
}

function Table({ node, scope, rows, columns, labels, title, sort, setSort, tree }: RowsProps & { tree: boolean }) {
  const app = useApp();
  const key = rowKey(node);
  const link = useRowLink(node, scope);
  const open = openAction(node.row_actions);
  const menuActions = (node.row_actions ?? []).filter((a: Action) => a !== open);
  const hasMenu = menuActions.length > 0;
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const sortable = (f: Field) => f.sortable || (node.sort?.allowed ?? []).includes(f.field);
  const colCount = columns.length + (hasMenu ? 1 : 0) + (node.expand ? 1 : 0);
  const groupBy: string | undefined = node.group_by;
  const groups = useMemo(() => {
    if (!groupBy) return [{ key: '', rows }];
    const order: string[] = [...(node.group_order ?? [])];
    const map = new Map<string, Json[]>();
    for (const r of rows) {
      const g = String(r?.[groupBy] ?? '');
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(r);
      if (!order.includes(g)) order.push(g);
    }
    return order.filter((g) => node.show_empty_groups || map.has(g)).map((g) => ({ key: g, rows: map.get(g) ?? [] }));
  }, [rows, groupBy, node.group_order, node.show_empty_groups]);
  const groupField = groupBy ? (node.columns ?? []).find((c: Field) => c.field === groupBy) : undefined;

  const body = (rs: Json[]) => {
    const ordered = tree ? treeOrder(rs, key) : rs.map((row) => ({ row, depth: 0 }));
    return ordered.map(({ row, depth }) => {
      const id = String(row?.[key] ?? JSON.stringify(row));
      const s = { ...scope, row };
      const to = link(row);
      const isOpen = expanded.has(id);
      const actions = visibleActions(menuActions, s);
      return (
        <Fragment key={id}>
          <tr className={to ? 'row-link' : undefined} onClick={to ? (e) => { if (!(e.target as HTMLElement).closest('a,button,select,input')) app.go(to); } : undefined}>
            {node.expand && (
              <td className="cell-expand">
                <button type="button" className="btn btn-ghost btn-icon btn-sm" aria-expanded={isOpen} aria-label={`${isOpen ? 'Collapse' : 'Expand'} ${rowName(row, columns, key)}`} onClick={() => setExpanded((x) => { const n = new Set(x); if (n.has(id)) n.delete(id); else n.add(id); return n; })}>
                  <Icon name={isOpen ? 'chevron_down' : 'chevron_right'} size={15} />
                </button>
              </td>
            )}
            {columns.map((f, i) => {
              const raw = row?.[f.field];
              const prose = !f.as && typeof raw === 'string' && raw.length > 24 && !/^\S+$/.test(raw);
              const cls = [cellClass(f), prose ? 'cell-prose' : undefined].filter(Boolean).join(' ') || undefined;
              const content = cellValue(f, row, labels, scope, i === 0 && !!to);
              return i === 0 ? (
                <th key={f.name} scope="row" className={cls} style={tree && depth ? { paddingInlineStart: `calc(0.75rem + ${depth * 1.25}rem)` } : undefined}>
                  {tree && depth > 0 && <span className="tree-branch" aria-hidden="true">└</span>}
                  {to ? <a href={to} className="cell-link">{content}</a> : content}
                </th>
              ) : <td key={f.name} className={cls}>{content}</td>;
            })}
            {hasMenu && <td className="cell-actions">{actions.length > 0 && <RowMenu actions={actions} scope={s} rowLabel={rowName(row, columns, key)} />}</td>}
          </tr>
          {node.expand && isOpen && (
            <tr className="row-expanded">
              <td colSpan={colCount}>
                <NodeView node={node.expand} scope={s} />
              </td>
            </tr>
          )}
        </Fragment>
      );
    });
  };

  return (
    <ScrollRegion className="table-wrap" label={title ?? 'Rows'}>
      <table className="table">
        {title && <caption className="sr-only">{title}</caption>}
        <thead>
          <tr>
            {node.expand && <td className="cell-expand" />}
            {columns.map((f) => {
              const s = sortable(f);
              const active = sort.by === f.field;
              return (
                <th key={f.name} scope="col" className={cellClass(f)} aria-sort={active ? (sort.dir === 'asc' ? 'ascending' : 'descending') : undefined}>
                  {s ? (
                    <button type="button" className="th-sort" onClick={() => setSort({ by: f.field, dir: active && sort.dir === 'asc' ? 'desc' : 'asc' })}>
                      {fieldLabel(f)}
                      <Icon name={active ? (sort.dir === 'asc' ? 'sort_up' : 'sort_down') : 'sort'} size={14} />
                    </button>
                  ) : fieldLabel(f)}
                </th>
              );
            })}
            {hasMenu && <td className="cell-actions" />}
          </tr>
        </thead>
        {groups.map((g) => (
          <tbody key={g.key || '_'}>
            {groupBy && (
              <tr className="group-row">
                <th scope="colgroup" colSpan={colCount}>
                  <GroupLabel value={g.key} field={groupField} labels={labels} />
                  <span className="count">{g.rows.length}</span>
                </th>
              </tr>
            )}
            {body(g.rows)}
          </tbody>
        ))}
      </table>
    </ScrollRegion>
  );
}

function GroupLabel({ value, field, labels }: { value: string; field?: Field; labels: LabelMaps }) {
  const app = useApp();
  if (field?.as === 'badge' || field?.tone_by) return <Badge tone={toneFor(field, value, app.doc.tone_maps)}>{words(value)}</Badge>;
  const l = field ? labels[field.name]?.get(value) : undefined;
  return <span className="group-label">{l ?? (value ? words(value) : 'None')}</span>;
}

function cellClass(f: Field): string | undefined {
  if (['number', 'bytes', 'duration', 'percent'].includes(f.as ?? '')) return 'cell-num';
  if (f.host) return 'cell-meter';
  if (f.as === 'list' || f.as === 'map' || f.as === 'tags') return 'cell-wide';
  return undefined;
}

function Cards({ node, scope, rows, columns, labels }: RowsProps) {
  const key = rowKey(node);
  const link = useRowLink(node, scope);
  const open = openAction(node.row_actions);
  const [head, ...rest] = columns;
  const badges = rest.filter((f) => f.as === 'badge');
  const others = rest.filter((f) => f.as !== 'badge');
  return (
    <ul className="cards">
      {rows.map((row) => {
        const s = { ...scope, row };
        const to = link(row);
        const actions = visibleActions((node.row_actions ?? []).filter((a: Action) => a !== open), s);
        return (
          <li key={String(row?.[key])} className="tile">
            <div className="tile-head">
              <h3 className="tile-title">{to ? <a href={to}>{head ? cellValue(head, row, labels, scope, true) : String(row?.[key])}</a> : head ? cellValue(head, row, labels, scope) : null}</h3>
              <div className="tile-badges">{badges.map((f) => <Fragment key={f.name}>{cellValue(f, row, labels, scope)}</Fragment>)}</div>
            </div>
            {others.length > 0 && (
              <dl className="tile-fields">
                {others.map((f) => (
                  <div key={f.name} className={f.field === 'summary' ? 'tile-summary' : undefined}>
                    <dt>{fieldLabel(f)}</dt>
                    <dd>{cellValue(f, row, labels, scope)}</dd>
                  </div>
                ))}
              </dl>
            )}
            {actions.length > 0 && <div className="tile-actions"><ActionBar actions={actions} scope={s} size="sm" primary handed={row} /></div>}
          </li>
        );
      })}
    </ul>
  );
}

function List({ node, scope, rows, columns, labels }: RowsProps) {
  const key = rowKey(node);
  const link = useRowLink(node, scope);
  const open = openAction(node.row_actions);
  return (
    <ul className="rowlist">
      {rows.map((row) => {
        const s = { ...scope, row };
        const to = link(row);
        const actions = visibleActions((node.row_actions ?? []).filter((a: Action) => a !== open), s);
        return (
          <li key={String(row?.[key])} className="rowlist-item">
            <dl className="props">
              {columns.map((f) => (
                <div key={f.name} className="prop">
                  <dt>{fieldLabel(f)}</dt>
                  <dd>{cellValue(f, row, labels, scope)}</dd>
                </div>
              ))}
            </dl>
            {(to || actions.length > 0) && (
              <div className="rowlist-actions">
                {to && <a className="btn btn-secondary btn-sm" href={to}>{open?.label ?? 'Open'}</a>}
                <ActionBar actions={actions} scope={s} size="sm" handed={row} />
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
