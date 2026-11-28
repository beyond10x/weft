// One value of a row as a field shows it (`Field.as`, `tone_by`, `labels`, `label_from`, a
// field-scope host slot): badges, numbers, bytes, durations, times, lists of resource amounts,
// record maps and long identifiers.

import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { labelFromParams, useApp } from '../runtime.tsx';
import { Badge, IdText } from '../../ui/kit.tsx';
import * as fmt from '../format.ts';
import type { Field, Json, Tone } from '../types.ts';

export type LabelMaps = Record<string, Map<string, string>>;

/**
 * The `label_from` maps of a list of fields, keyed by field name; each view read once, with the
 * params the renderer binds from the actor and the page (`labelFromParams`, a convention: ess-ui/1
 * reads it without params). A view whose params have no value gets no map.
 */
export function useLabelMaps(fields: Field[], pageParams?: Record<string, Json>): LabelMaps {
  const app = useApp();
  const specs = fields
    .filter((f) => f.label_from)
    .map((f) => ({ name: f.name, ...f.label_from!, params: labelFromParams(app.bundle.model, f.label_from!.view, app.actor, pageParams) }))
    .filter((s) => s.params !== undefined) as { name: string; view: string; field: string; key?: string; params: Record<string, Json> }[];
  const sig = JSON.stringify(specs);
  const build = (rowsOf: (v: string) => Json[] | undefined): LabelMaps => {
    const out: LabelMaps = {};
    for (const s of specs) {
      const rows = rowsOf(s.view);
      if (rows) out[s.name] = new Map(rows.map((r) => [String(r?.[s.key ?? 'id']), String(r?.[s.field] ?? '')]));
    }
    return out;
  };
  const [maps, setMaps] = useState<LabelMaps>(() => {
    const peeked = new Map(specs.map((s) => [s.view, app.peekView(s.view, s.params)?.rows] as const));
    return build((v) => peeked.get(v));
  });
  useEffect(() => {
    if (!specs.length) return;
    let live = true;
    Promise.all(specs.map((s) => app.readView(s.view, s.params).then((r) => [s.view, r.rows] as const, () => [s.view, undefined] as const))).then((all) => {
      if (!live) return;
      const by = new Map(all);
      setMaps(build((v) => by.get(v)));
    });
    return () => { live = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig]);
  return maps;
}

export function toneFor(f: Field, v: Json, toneMaps: Record<string, Json> | undefined): Tone {
  const map = f.tone_by ? toneMaps?.[f.tone_by] : undefined;
  return fmt.toneOf(v, map, f.tone ?? 'neutral');
}

export function labelFor(f: Field, v: Json, labelMaps: Record<string, Record<string, string>> | undefined): string | undefined {
  const map = typeof f.labels === 'string' ? labelMaps?.[f.labels] : f.labels;
  if (!map) return undefined;
  const k = v === null || v === undefined ? 'null' : String(v);
  return map[k];
}

const isEmpty = (v: Json) => v === null || v === undefined || v === '' || (Array.isArray(v) && v.length === 0);

/** A value with no field (a nested value of a list or a record map). */
export function Plain({ v, noCopy }: { v: Json; noCopy?: boolean }): ReactNode {
  const app = useApp();
  const copy = (x: string) => { void navigator.clipboard?.writeText(x).catch(() => undefined); app.notify('info', 'Copied', x); };
  if (isEmpty(v)) return <span className="muted">—</span>;
  if (typeof v === 'boolean') return <span>{v ? 'yes' : 'no'}</span>;
  if (typeof v === 'number') return <span className="num">{fmt.number(v)}</span>;
  if (typeof v === 'string') {
    if (fmt.UUID.test(v) || fmt.DIGEST.test(v)) return <IdText value={v} onCopy={noCopy ? undefined : copy} />;
    if (/^\d{4}-\d\d-\d\dT\d\d:\d\d/.test(v)) return <time dateTime={v} title={fmt.ago(v)}>{fmt.timestamp(v)}</time>;
    if (/^\d+(ms|s|m|h)$/.test(v)) return <span>{fmt.durationText(v)}</span>;
    return <span>{fmt.words(v)}</span>;
  }
  if (Array.isArray(v)) return <ChipList items={v} />;
  return <KeyValues obj={v} />;
}

function ChipList({ items }: { items: Json[] }) {
  return (
    <ul className="chips">
      {items.map((x, i) => (
        <li key={i} className="chip">{chipText(x)}</li>
      ))}
    </ul>
  );
}

function chipText(x: Json): ReactNode {
  if (x === null || typeof x !== 'object') return typeof x === 'string' && (fmt.UUID.test(x) || fmt.DIGEST.test(x)) ? <code>{fmt.shortId(x)}</code> : fmt.words(String(x));
  if ('resource_class' in x && 'amount' in x) return <><span className="chip-key">{x.resource_class}</span> <b>{fmt.number(x.amount)}</b></>;
  if ('class' in x && 'amount' in x) return <><span className="chip-key">{x.class}</span> <b>× {fmt.number(x.amount)}</b>{x.name ? <span className="chip-sub"> {x.name}</span> : null}</>;
  if ('name' in x && 'role' in x) return <><b>{x.name}</b> <span className="chip-sub">{x.role}</span></>;
  return fmt.plain(x);
}

function KeyValues({ obj }: { obj: Json }) {
  const entries = Object.entries(obj).filter(([, x]) => !isEmpty(x));
  if (!entries.length) return <span className="muted">—</span>;
  return (
    <dl className="kv">
      {entries.map(([k, x]) => (
        <div key={k} className="kv-pair">
          <dt>{k.replace(/_/g, ' ')}</dt>
          <dd>{typeof x === 'object' && x !== null && !Array.isArray(x) ? fmt.plain(x) : Array.isArray(x) ? x.map((y) => (typeof y === 'object' ? fmt.plain(y) : fmt.words(String(y)))).join(', ') : typeof x === 'boolean' ? (x ? 'yes' : 'no') : typeof x === 'string' && /^\d+(ms|s|m|h)$/.test(x) ? fmt.durationText(x) : fmt.words(String(x))}</dd>
        </div>
      ))}
    </dl>
  );
}

/** One field of one row. */
export function FieldValue({ f, row, labels, noCopy }: { f: Field; row: Json; labels?: LabelMaps; noCopy?: boolean }) {
  const app = useApp();
  const doc = app.doc;
  const v = f.field === '.' ? (typeof row === 'object' ? undefined : row) : row?.[f.field];
  if (f.host) {
    const Host = app.hosts[f.host];
    if (!Host) return <span className="muted">no host component {f.host}</span>;
    return <Host slot={f} row={row} value={v} label={f.label ?? f.field} data={app.bundle.hosts[f.host]} />;
  }
  const shown = labelFor(f, v, doc.label_maps);
  const fromLabel = labels?.[f.name]?.get(String(v));
  if (f.as === 'badge' || f.as === 'choice') {
    if (isEmpty(v)) return <span className="muted">—</span>;
    return <Badge tone={toneFor(f, v, doc.tone_maps)}>{shown ?? fromLabel ?? fmt.words(String(v))}</Badge>;
  }
  if (isEmpty(v)) return <span className="muted">—</span>;
  if (fromLabel) return <span className="label-from" title={String(v)}>{fromLabel}</span>;
  if (shown !== undefined) return <span>{shown}</span>;
  const toned = (node: ReactNode) => (f.tone_by || (f.tone && f.tone !== 'neutral') ? <span className={`tone-text tone-${toneFor(f, v, doc.tone_maps)}`}>{node}</span> : node);
  switch (f.as) {
    case 'number': return toned(<span className="num">{fmt.number(v)}</span>);
    case 'bytes': return toned(<span className="num">{fmt.bytes(Number(v))}</span>);
    case 'duration': return toned(<span className="num">{typeof v === 'number' ? fmt.duration(v, f.unit ?? 'ms') : fmt.durationText(String(v))}</span>);
    case 'percent': return toned(<span className="num">{fmt.percent(Number(v), f.scale)}</span>);
    case 'timestamp': return <time dateTime={String(v)} title={fmt.ago(v)}>{fmt.timestamp(v)}</time>;
    case 'date': return <time dateTime={String(v)}>{fmt.date(v)}</time>;
    case 'time': return <time dateTime={String(v)}>{fmt.time(v)}</time>;
    case 'secret': return <span className="muted">••••••</span>;
    case 'json': return <pre className="json">{JSON.stringify(v, null, 2)}</pre>;
    case 'list':
    case 'tags': return Array.isArray(v) ? <ChipList items={v} /> : <Plain v={v} />;
    case 'map': return typeof v === 'object' ? <KeyValues obj={v} /> : <Plain v={v} />;
    default: return toned(<Plain v={v} noCopy={noCopy} />);
  }
}
