// The host components of the two consoles: what the documents' host slots (`host: <Name>`) stand
// in for, because ess-ui/1 has no node for them. Meters draw a value against its limit; LogTail
// draws the lines of an instance's log streams (fixture mode: recorded lines).

import { useMemo, useState } from 'react';
import type { HostComponent, HostProps } from '../renderer/runtime.tsx';
import { Meter, EmptyState } from '../ui/kit.tsx';
import { Icon } from '../ui/Icon.tsx';
import * as fmt from '../renderer/format.ts';
import type { Json } from '../renderer/types.ts';

const n = (v: Json) => (typeof v === 'number' ? v : Number(v ?? 0));

function rowName(p: HostProps): string {
  return p.label ?? 'Value';
}

/** Desired size of a node pool between its min and max. */
const PoolSizeMeter: HostComponent = (p) => {
  const r = p.row ?? {};
  return <Meter kind="usage" value={n(r.desired_size)} min={n(r.min_size)} max={n(r.max_size)} label={`Desired size of ${r.name ?? 'pool'}`} text={`${n(r.desired_size)} · ${n(r.min_size)}–${n(r.max_size)}`} />;
};

/** A quota: used (limit − remaining) against its limit. */
const QuotaMeter: HostComponent = (p) => {
  const r = p.row ?? {};
  const limit = n(r.limit);
  const used = limit - n(r.remaining);
  return <Meter value={used} max={limit} label={`${r.resource_class ?? 'Quota'} used`} text={`${fmt.number(used)} of ${fmt.number(limit)}`} />;
};

/** Running replicas against wanted replicas. */
const ReplicaMeter: HostComponent = (p) => {
  const r = p.row ?? {};
  const want = n(r.replicas);
  const run = n(r.running_replicas);
  return <Meter kind="goal" value={run} max={want} label={`${rowName(p)} running`} text={want === 0 ? 'asleep · 0' : `${run} / ${want}`} />;
};

/** A rollout step: ready replicas against the step's replicas. */
const StepMeter: HostComponent = (p) => {
  const r = p.row ?? {};
  if (r.replicas === null || r.replicas === undefined) return <span className="muted">—</span>;
  return <Meter kind="goal" value={n(r.ready_replicas)} max={n(r.replicas)} label="Ready replicas" text={`${n(r.ready_replicas)} / ${n(r.replicas)}`} />;
};

/** A budget: spent against its amount. */
const BudgetMeter: HostComponent = (p) => {
  const r = p.row ?? {};
  const amount = n(r.amount);
  const spent = n(r.spent);
  return <Meter value={spent} max={amount} label={`Spent of the ${fmt.words(String(r.measure ?? ''))} budget`} text={`${fmt.number(spent)} of ${fmt.number(amount)}`} />;
};

interface Line { at: string; stream: string; text: string }

/** The lines of one instance's log streams, newest last, in a terminal panel. */
const LogTail: HostComponent = (p) => {
  const streams: Json[] = useMemo(() => [...(p.rows ?? [])].sort((a, b) => (a.state === 'Open' ? 0 : 1) - (b.state === 'Open' ? 0 : 1)), [p.rows]);
  const [at, setAt] = useState<string | undefined>(streams[0]?.log_stream_id);
  const [only, setOnly] = useState<'all' | 'stderr'>('all');
  const lines: Line[] = (p.data?.streams?.[at ?? ''] ?? []) as Line[];
  const shown = only === 'all' ? lines : lines.filter((l) => l.stream === 'stderr');
  if (!streams.length) return <EmptyState message="This instance has no log stream." />;
  return (
    <div className="logtail">
      <div className="logtail-bar">
        <label className="filter-choice">
          <span className="filter-label">Stream</span>
          <select value={at} onChange={(e) => setAt(e.target.value)}>
            {streams.map((s) => (
              <option key={s.log_stream_id} value={s.log_stream_id}>
                {fmt.words(String(s.kind))}{s.process ? ` · ${s.process}` : ''} · {fmt.words(String(s.state))}
              </option>
            ))}
          </select>
        </label>
        <fieldset className="segmented">
          <legend className="sr-only">Lines</legend>
          {(['all', 'stderr'] as const).map((k) => (
            <label key={k} className={`seg${only === k ? ' seg-on' : ''}`}>
              <input type="radio" name="logtail-only" checked={only === k} onChange={() => setOnly(k)} />
              <span>{k === 'all' ? 'All lines' : 'stderr only'}</span>
            </label>
          ))}
        </fieldset>
        <span className="logtail-note"><Icon name="terminal" size={15} /> Recorded lines, fixture mode</span>
      </div>
      {shown.length ? (
        <div className="logtail-lines" role="log" aria-label="Log lines" tabIndex={0}>
          <ol>
            {shown.map((l, i) => (
              <li key={i} className={l.stream === 'stderr' ? 'stderr' : undefined}>
                <time dateTime={l.at}>{fmt.time(l.at)}</time>
                <span className="logtail-stream">{l.stream}</span>
                <span className="logtail-text">{l.text}</span>
              </li>
            ))}
          </ol>
        </div>
      ) : (
        <EmptyState message="No line in this stream yet." />
      )}
    </div>
  );
};

export const operatorHosts: Record<string, HostComponent> = {
  PoolSizeMeter,
  TenantQuotaMeter: QuotaMeter,
  QuotaMeter,
};

export const accountHosts: Record<string, HostComponent> = {
  HomeReplicaMeter: ReplicaMeter,
  ServiceListMeter: ReplicaMeter,
  ServiceDetailMeter: ReplicaMeter,
  StepMeter,
  BudgetMeter,
  LogTail,
};
