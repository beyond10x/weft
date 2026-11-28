// Maps one ess-ui/1 node to its component: a host slot to the host's component, a built-in
// component by name, a primitive, and anything else to a visible placeholder naming what is missing.

import { useEffect } from 'react';
import type { ReactNode } from 'react';
import { useApp, useRead } from '../runtime.tsx';
import type { Scope } from '../expr.ts';
import { value } from '../expr.ts';
import { Collection } from './Collection.tsx';
import type { RowsReport } from './Collection.tsx';
import { RecordView } from './Record.tsx';
import { ChartView, MetricView } from './Metric.tsx';
import { FilterBar } from './FilterBar.tsx';
import { FormView } from './Form.tsx';
import { ReadStates } from './states.tsx';
import { ActionBar } from './actions.tsx';
import { Badge, EmptyState } from '../../ui/kit.tsx';
import { Icon } from '../../ui/Icon.tsx';
import type { Obj } from '../types.ts';

export interface NodeProps {
  node: Obj;
  scope: Scope;
  title?: string;
  report?: (r: RowsReport) => void;
  onDone?: () => void;
}

function HostSlot({ node, scope, report }: NodeProps) {
  const app = useApp();
  const read = useRead(node.reads, scope);
  const Host = app.hosts[node.host];
  useEffect(() => { report?.({ status: read.status, rows: read.rows, total: read.total }); }, [read.status, read.rows, read.total, report]);
  if (!Host) return <EmptyState title="Host component missing" message={`This renderer has no component for the host slot ${node.host}.`} />;
  if (!node.reads) return <Host slot={node} data={app.bundle.hosts[node.host]} />;
  return (
    <ReadStates read={read} states={node.states} scope={scope}>
      {() => <Host slot={node} rows={read.rows} data={app.bundle.hosts[node.host]} />}
    </ReadStates>
  );
}

export function NodeView(p: NodeProps): ReactNode {
  const { node, scope } = p;
  if (node.host) return <HostSlot {...p} />;
  switch (node.component) {
    case 'collection': return <Collection node={node} scope={scope} title={p.title} report={p.report} />;
    case 'record': return <RecordView node={node} scope={scope} report={p.report} />;
    case 'metric': return <MetricView node={node} scope={scope} report={p.report} />;
    case 'chart': return <ChartView node={node} scope={scope} title={p.title} report={p.report} />;
    case 'filter_bar': return <FilterBar node={node} scope={scope} />;
    case 'form': return <FormView node={node} scope={scope} onDone={p.onDone} />;
    case 'text': return <p className="text">{String(value(node.text ?? node.value, scope) ?? '')}</p>;
    case 'badge': return <Badge tone={node.tone ?? 'neutral'}>{String(value(node.text, scope) ?? '')}</Badge>;
    case 'icon': return <Icon name={node.icon ?? node.name} label={node.label} />;
    case 'divider': return <hr className="divider" />;
    case 'button':
    case 'link': return node.action ? <ActionBar actions={[node.action]} scope={scope} /> : null;
    case 'stack':
    case 'row':
      return (
        <div className={node.component === 'row' ? 'node-row' : 'node-stack'}>
          {(node.children ?? []).map((c: Obj, i: number) => <NodeView key={c.name ?? i} node={c} scope={scope} />)}
        </div>
      );
    default:
      return <EmptyState title="Not drawn" message={`This renderer has no component ${String(node.component)}.`} />;
  }
}
