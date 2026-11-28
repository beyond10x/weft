// Answers reads from a document's fixtures, the way `ess ui run` does without a backend: a view's
// rows from its fixture file, `derived` views from another view (`same_as`, or `by_id_from` keyed
// by the read's param of that key's name). A fixture holds one row set per view, so rows are kept
// when every read param that names a field of the row equals it (InstanceList(workload_id) keeps
// the rows of that workload); a param that names no row field (a time window) keeps every row.

import type { Fixtures, Json } from './types.ts';

export class NoFixture extends Error {
  constructor(public view: string) {
    super(`No fixture answers ${view}`);
  }
}

export interface ReadResult {
  rows: Json[];
  total: number;
}

const defined = (v: Json) => v !== undefined && v !== null && v !== '';

export function matchParams(row: Json, params: Record<string, Json>): boolean {
  if (row === null || typeof row !== 'object') return true;
  for (const [k, v] of Object.entries(params)) {
    if (!defined(v) || !(k in row)) continue;
    if (String(row[k]) !== String(v)) return false;
  }
  return true;
}

export class FixtureServer {
  constructor(private fx: Fixtures) {}

  has(view: string): boolean {
    return view in this.fx.views || view in this.fx.derived;
  }

  /** The rows of `view` for `params`; throws NoFixture when no fixture answers it. */
  read(view: string, params: Record<string, Json> = {}, depth = 0): ReadResult {
    if (depth > 8) throw new NoFixture(view);
    const own = this.fx.views[view];
    if (own) {
      const rows = own.rows.filter((r) => matchParams(r, params));
      return { rows, total: rows.length === own.rows.length && own.total !== undefined ? own.total : rows.length };
    }
    const d = this.fx.derived[view];
    if (d?.same_as) return this.read(d.same_as, params, depth + 1);
    if (d?.by_id_from) {
      const key = d.key ?? 'id';
      const from = this.read(d.by_id_from, {}, depth + 1).rows;
      const id = params[key] ?? params.id;
      const rows = from.filter((r) => String(r?.[key]) === String(id));
      return { rows, total: rows.length };
    }
    throw new NoFixture(view);
  }
}
