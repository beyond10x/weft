// Actions (`Action`): run a command with its `bind`, open an overlay with the opener's values,
// navigate, set UI state, copy, or export a view as CSV. A `confirm` asks first.

import { useCallback } from 'react';
import { useApp, usePage } from '../runtime.tsx';
import { holds, value, values } from '../expr.ts';
import type { Scope } from '../expr.ts';
import { href } from '../route.ts';
import { plain } from '../format.ts';
import { Button, Menu } from '../../ui/kit.tsx';
import type { ButtonKind } from '../../ui/kit.tsx';
import type { Action, Json } from '../types.ts';

export function actionLabel(a: Action): string {
  if (a.label) return a.label;
  const n = a.name.replace(/_/g, ' ');
  return n.charAt(0).toUpperCase() + n.slice(1);
}

function toCsv(rows: Json[]): string {
  const cols = Array.from(rows.reduce<Set<string>>((s, r) => { Object.keys(r ?? {}).forEach((k) => s.add(k)); return s; }, new Set()));
  const cell = (v: Json) => {
    const t = plain(v);
    return /[",\n]/.test(t) ? `"${t.replace(/"/g, '""')}"` : t;
  };
  return [cols.join(','), ...rows.map((r) => cols.map((c) => cell(r?.[c])).join(','))].join('\n');
}

/** Runs actions in a scope. `handed` is what an overlay opened here receives (a row, the page params). */
export function useRunAction() {
  const app = useApp();
  const page = usePage();
  return useCallback(async (a: Action, scope: Scope, handed?: Record<string, Json>) => {
    if (a.confirm && !a.opens) {
      const c = typeof a.confirm === 'string' ? { title: a.confirm } : a.confirm;
      const ok = await app.confirm({ title: c.title, confirmLabel: c.confirm_label, danger: true, body: a.does ? `Runs ${a.does}.` : undefined });
      if (!ok) return;
    }
    if (a.does) {
      app.runCommand(a.does, values(a.bind, scope));
    } else if (a.opens) {
      page.openOverlay(a.opens, handed ?? scope.row ?? { ...page.params });
    } else if (a.navigate) {
      app.go(href(app.doc, a.navigate.to, values(a.navigate.params, scope)));
    } else if (a.sets) {
      for (const [k, v] of Object.entries(a.sets)) {
        const val = v === 'toggle' ? !value(k, scope) : value(v, scope);
        if (k.startsWith('shell.')) app.setShell(k.slice(6), val);
        else if (k.startsWith('state.')) page.setState(k.slice(6), val);
      }
    } else if (a.copy) {
      const text = String(value(a.copy, scope) ?? '');
      await navigator.clipboard?.writeText(text).catch(() => undefined);
      app.notify('info', 'Copied', text);
    } else if (a.export) {
      const params = typeof a.export.params === 'object' ? values(a.export.params, scope) : {};
      try {
        const res = await app.readView(a.export.reads, params);
        const blob = new Blob([a.export.as === 'json' ? JSON.stringify(res.rows, null, 2) : toCsv(res.rows)], { type: a.export.as === 'json' ? 'application/json' : 'text/csv' });
        const link = document.createElement('a');
        link.href = URL.createObjectURL(blob);
        link.download = `${a.export.reads.split('.').pop()}.${a.export.as === 'json' ? 'json' : 'csv'}`;
        link.click();
        URL.revokeObjectURL(link.href);
        app.notify('success', 'Exported', `${res.rows.length} rows of ${a.export.reads}`);
      } catch (e) {
        app.notify('danger', 'Export failed', String((e as Error)?.message ?? e));
      }
    } else if (a.upload) {
      app.notify('info', 'Upload', 'File upload is not available in fixture mode.');
    }
  }, [app, page]);
}

export function visibleActions(actions: Action[] | undefined, scope: Scope): Action[] {
  return (actions ?? []).filter((a) => holds(a.visible, scope));
}

/** A row of action buttons; the first is primary when `primary` is set. */
export function ActionBar({ actions, scope, primary, size, handed }: { actions?: Action[]; scope: Scope; primary?: boolean; size?: 'sm'; handed?: Record<string, Json> }) {
  const run = useRunAction();
  const shown = visibleActions(actions, scope);
  if (!shown.length) return null;
  return (
    <>
      {shown.map((a, i) => {
        const kind: ButtonKind = primary && i === 0 ? 'primary' : a.confirm && a.does ? 'secondary' : 'secondary';
        return <Button key={a.name} kind={kind} size={size} onClick={() => void run(a, scope, handed)}>{actionLabel(a)}</Button>;
      })}
    </>
  );
}

/** The actions of a row in a menu, labelled by the row. */
export function RowMenu({ actions, scope, rowLabel }: { actions: Action[]; scope: Scope; rowLabel: string }) {
  const run = useRunAction();
  if (!actions.length) return null;
  return (
    <Menu
      label={`Actions for ${rowLabel}`}
      items={actions.map((a) => ({ key: a.name, label: actionLabel(a), danger: !!a.confirm && !!a.does, onSelect: () => void run(a, scope, scope.row) }))}
    />
  );
}
