// Value formats of ess-ui/1 fields and metrics (`Field.as`, `metric.format`), and the text a value
// shows as. Times are shown in UTC, as the schema says for `time` and `timestamp`.

import type { Json, Tone } from './types.ts';

const UNIT_MS = { ms: 1, s: 1000, m: 60_000, h: 3_600_000 } as const;

/** `bytes`: B, KB, MB or GB (and TB) in 1024 steps. */
export function bytes(n: number): string {
  if (!Number.isFinite(n)) return String(n);
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  let v = Math.abs(n);
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  const s = i === 0 ? String(v) : v >= 100 ? v.toFixed(0) : v.toFixed(1).replace(/\.0$/, '');
  return `${n < 0 ? '-' : ''}${s} ${units[i]}`;
}

/** `duration`: h:mm:ss in whole seconds, hours unpadded, never folded into days. */
export function duration(n: number, unit: keyof typeof UNIT_MS = 'ms'): string {
  if (!Number.isFinite(n)) return String(n);
  const total = Math.floor(Math.abs(n) * UNIT_MS[unit] / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return `${n < 0 ? '-' : ''}${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

/** A duration value of the model (`15m`, `30s`, `2h`) in words. */
export function durationText(v: string): string {
  const m = /^(\d+)(ms|s|m|h)$/.exec(v);
  if (!m) return v;
  const n = Number(m[1]);
  const word = { ms: 'ms', s: 's', m: 'min', h: 'h' }[m[2] as 'ms'];
  return `${n} ${word}`;
}

export function percent(n: number, scale: 'fraction' | 'hundredths' = 'fraction'): string {
  const v = scale === 'fraction' ? n * 100 : n;
  return `${Number.isInteger(v) ? v : v.toFixed(1)}%`;
}

const NUM = new Intl.NumberFormat('en-US', { maximumFractionDigits: 2 });
export function number(n: Json): string {
  const v = typeof n === 'string' ? Number(n) : n;
  return typeof v === 'number' && Number.isFinite(v) ? NUM.format(v) : String(n);
}

const TS = new Intl.DateTimeFormat('en-GB', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', hour12: false, timeZone: 'UTC' });
const DATE = new Intl.DateTimeFormat('en-GB', { day: '2-digit', month: 'short', year: 'numeric', timeZone: 'UTC' });
const TIME = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false, timeZone: 'UTC' });

export function timestamp(v: Json): string {
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? String(v) : `${TS.format(d).replace(',', '')} UTC`;
}
export function date(v: Json): string {
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? String(v) : DATE.format(d);
}
export function time(v: Json): string {
  const d = new Date(v);
  return Number.isNaN(d.getTime()) ? String(v) : TIME.format(d);
}

/** "3 h ago", for the title of a timestamp. */
export function ago(v: Json, now: number = Date.now()): string {
  const t = new Date(v).getTime();
  if (Number.isNaN(t)) return '';
  const s = Math.round((now - t) / 1000);
  const abs = Math.abs(s);
  const [n, u] = abs < 60 ? [abs, 's'] : abs < 3600 ? [Math.round(abs / 60), 'min'] : abs < 86400 ? [Math.round(abs / 3600), 'h'] : [Math.round(abs / 86400), 'd'];
  return s >= 0 ? `${n} ${u} ago` : `in ${n} ${u}`;
}

export const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const DIGEST = /^sha256:[0-9a-f]{16,}$/i;

/** A short form of a long identifier: the first 8 hex digits of a UUID, a digest's first 12. */
export function shortId(v: string): string {
  if (UUID.test(v)) return v.slice(0, 8);
  if (DIGEST.test(v)) return `${v.slice(0, 19)}…`;
  return v;
}

/** A value as plain text (for sorting, filtering, titles and the charts' tables). */
export function plain(v: Json): string {
  if (v === null || v === undefined) return '';
  if (Array.isArray(v)) return v.map(plain).join(', ');
  if (typeof v === 'object') {
    if ('resource_class' in v && 'amount' in v) return `${v.resource_class} ${number(v.amount)}`;
    return Object.entries(v).map(([k, x]) => `${k} ${plain(x)}`).join(', ');
  }
  return String(v);
}

/** A tone for a value: a value map (exact keys) or bands over numbers; `fallback` otherwise. */
export function toneOf(v: Json, map: Json, fallback: Tone = 'neutral'): Tone {
  if (!map) return fallback;
  if (Array.isArray(map.bands)) {
    const n = typeof v === 'number' ? v : Number(v);
    if (!Number.isFinite(n)) return fallback;
    let t: Tone | undefined;
    for (const b of map.bands) if (n >= b.from) t = b.tone;
    return t ?? fallback;
  }
  const k = v === null || v === undefined ? 'null' : String(v);
  return (map[k] as Tone) ?? fallback;
}

/** Spaces a variant spelling for reading: `NoCapacity` → `No capacity`, `cpu-seconds` stays. */
export function words(v: string): string {
  if (!/^[A-Z][a-z]+[A-Z]/.test(v)) return v;
  const s = v.replace(/([a-z0-9])([A-Z])/g, '$1 $2');
  return s.charAt(0) + s.slice(1).toLowerCase();
}
