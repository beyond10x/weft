// `metric`: one number from a read (a field of its first row, or an aggregate over its rows), in
// its format; and `chart`: a series drawn as horizontal bars (bar), a line, or a doughnut, with the
// series as a table for screen readers.

import { useEffect } from 'react';
import { useRead } from '../runtime.tsx';
import type { RowsReport } from './Collection.tsx';
import type { Scope } from '../expr.ts';
import { value as evalValue } from '../expr.ts';
import * as fmt from '../format.ts';
import { ReadStates } from './states.tsx';
import type { Json, Obj } from '../types.ts';

export function aggregate(rows: Json[], agg: string | undefined, field: string | undefined): number | undefined {
  if (!agg) return undefined;
  if (agg === 'count') return rows.length;
  const xs = rows.map((r) => Number(r?.[field ?? ''])).filter((n) => Number.isFinite(n));
  if (!xs.length) return agg === 'sum' ? 0 : undefined;
  if (agg === 'sum') return xs.reduce((a, b) => a + b, 0);
  if (agg === 'min') return Math.min(...xs);
  if (agg === 'max') return Math.max(...xs);
  return xs.reduce((a, b) => a + b, 0) / xs.length;
}

export function formatMetric(v: number | undefined, node: Obj): string {
  if (v === undefined || v === null || Number.isNaN(v)) return '—';
  switch (node.format) {
    case 'bytes': return fmt.bytes(v);
    case 'duration': return fmt.duration(v, node.unit ?? 's');
    case 'percent': return fmt.percent(v, node.scale);
    default: return fmt.number(Math.round(v * 100) / 100);
  }
}

export function MetricView({ node, scope, report }: { node: Obj; scope: Scope; report?: (r: RowsReport) => void }) {
  const read = useRead(node.reads, scope);
  useEffect(() => { report?.({ status: read.status, rows: read.rows, total: read.total }); }, [read.status, read.rows, read.total, report]);
  const label = node.label ?? node.name;
  if (node.from && !node.reads) {
    const v = evalValue(node.from, scope);
    return <Tile label={label} text={formatMetric(Number(v), node)} />;
  }
  const states = { ...node.states, empty: node.states?.empty };
  if (read.status === 'empty' && node.aggregate) return <Tile label={label} text={formatMetric(aggregate([], node.aggregate, node.field), node)} window={node.window} />;
  return (
    <ReadStates read={read} states={states} scope={scope} kind="tile">
      {() => {
        const v = node.aggregate ? aggregate(read.rows, node.aggregate, node.field) : Number(read.rows[0]?.[node.match ?? node.field ?? 'value']);
        return <Tile label={label} text={formatMetric(v, node)} window={node.window} />;
      }}
    </ReadStates>
  );
}

function Tile({ label, text, window }: { label: string; text: string; window?: string }) {
  return (
    <div className="metric">
      <p className="metric-value">{text}</p>
      <p className="metric-label">{label}{window ? <span className="muted"> · {fmt.durationText(window)}</span> : null}</p>
    </div>
  );
}

export function ChartView({ node, scope, title, report }: { node: Obj; scope: Scope; title?: string; report?: (r: RowsReport) => void }) {
  const read = useRead(node.reads, scope);
  useEffect(() => { report?.({ status: read.status, rows: read.rows, total: read.total }); }, [read.status, read.rows, read.total, report]);
  return (
    <ReadStates read={read} states={node.states} scope={scope} kind="chart">
      {() => <Bars rows={read.rows} x={node.x} series={node.series ?? []} title={title ?? node.name} />}
    </ReadStates>
  );
}

/** Rows with the same x are summed (a rollup by meter and qualifier draws one bar per meter). */
function Bars({ rows, x, series, title }: { rows: Json[]; x?: string; series: string[]; title: string }) {
  const y = series[0];
  const sums = new Map<string, number>();
  for (const r of rows) {
    const k = fmt.plain(x ? r?.[x] : '');
    sums.set(k, (sums.get(k) ?? 0) + Number(r?.[y] ?? 0));
  }
  const data = [...sums.entries()].sort((a, b) => b[1] - a[1]);
  const max = Math.max(...data.map((d) => d[1]), 0) || 1;
  const total = data.reduce((a, d) => a + d[1], 0);
  return (
    <figure className="chart">
      <ul className="bars" aria-hidden="true">
        {data.map(([k, v]) => (
          <li key={k} className="bar-row">
            <span className="bar-label">{k}</span>
            <span className="bar-track"><span className="bar-fill" style={{ width: `${(v / max) * 100}%` }} /></span>
            <span className="bar-value num">{fmt.number(Math.round(v * 100) / 100)}</span>
          </li>
        ))}
      </ul>
      <figcaption className="chart-caption">
        Total {y} {fmt.number(Math.round(total * 100) / 100)} over {data.length} {x ? `${x} values` : 'values'}
      </figcaption>
      <table className="sr-only">
        <caption>{title}</caption>
        <thead><tr><th scope="col">{x}</th><th scope="col">{y}</th></tr></thead>
        <tbody>{data.map(([k, v]) => <tr key={k}><th scope="row">{k}</th><td>{v}</td></tr>)}</tbody>
      </table>
    </figure>
  );
}
