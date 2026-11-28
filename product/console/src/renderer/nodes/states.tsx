// The section lifecycle drawn (`SectionStates`): a skeleton while loading, the empty state of the
// document, the failed state with a retry, and nothing yet for a read whose params have no value.

import type { ReactNode } from 'react';
import { EmptyState, ErrorState, Skeleton, Button } from '../../ui/kit.tsx';
import { actionLabel, useRunAction, visibleActions } from './actions.tsx';
import type { ReadState } from '../runtime.tsx';
import type { Scope } from '../expr.ts';
import type { Obj } from '../types.ts';

export function EmptyAction({ states, scope }: { states?: Obj; scope: Scope }) {
  const run = useRunAction();
  const a = states?.empty?.action;
  if (!a || !visibleActions([a], scope).length) return null;
  return <Button kind="primary" onClick={() => void run(a, scope)}>{actionLabel(a)}</Button>;
}

/** Draws `children` when the read holds rows, else the lifecycle state the document declares. */
export function ReadStates({ read, states, scope, kind = 'rows', children }: { read: ReadState; states?: Obj; scope: Scope; kind?: 'rows' | 'tile' | 'chart'; children: () => ReactNode }) {
  if (read.status === 'loading') return states?.loading === 'none' ? null : <Skeleton kind={kind} rows={4} />;
  if (read.status === 'failed') {
    const retry = states?.failed?.retry !== false;
    return <ErrorState message={states?.failed?.message ?? read.error ?? 'The read failed.'} onRetry={retry ? read.retry : undefined} />;
  }
  if (read.status === 'forbidden') return <EmptyState title="Not permitted" message={states?.forbidden?.message ?? 'Your grants do not include this view.'} />;
  if (read.status === 'idle' || read.status === 'empty') {
    const e = states?.empty;
    return <EmptyState title={e?.title} message={e?.message ?? 'Nothing to show yet'} action={<EmptyAction states={states} scope={scope} />} />;
  }
  return <>{children()}</>;
}
