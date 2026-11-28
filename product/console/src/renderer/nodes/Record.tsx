// `record`: the first row of a read as labelled fields, then `groups` under headings (collapsible),
// with record `actions` that open overlays handed the row.

import { useEffect } from 'react';
import { useRead } from '../runtime.tsx';
import type { Scope } from '../expr.ts';
import { holds } from '../expr.ts';
import { FieldValue, useLabelMaps } from './Value.tsx';
import { fieldLabel } from './Collection.tsx';
import type { RowsReport } from './Collection.tsx';
import { ActionBar } from './actions.tsx';
import { ReadStates } from './states.tsx';
import type { Field, Json, Obj } from '../types.ts';

function Fields({ fields, row, scope, labels }: { fields: Field[]; row: Json; scope: Scope; labels: ReturnType<typeof useLabelMaps> }) {
  const shown = fields.filter((f) => holds(f.visible, { ...scope, row }));
  return (
    <dl className="props">
      {shown.map((f) => (
        <div key={f.name} className={`prop${f.host ? ' prop-wide' : ''}`}>
          <dt>{fieldLabel(f)}</dt>
          <dd><FieldValue f={f} row={row} labels={labels} /></dd>
        </div>
      ))}
    </dl>
  );
}

export function RecordView({ node, scope, report }: { node: Obj; scope: Scope; report?: (r: RowsReport) => void }) {
  const read = useRead(node.reads, scope);
  const fields: Field[] = node.fields ?? [];
  const groups: Obj[] = node.groups ?? [];
  const labels = useLabelMaps([...fields, ...groups.flatMap((g) => g.fields as Field[])], scope.params);
  useEffect(() => { report?.({ status: read.status, rows: read.rows, total: read.total }); }, [read.status, read.rows, read.total, report]);
  return (
    <ReadStates read={read} states={node.states} scope={scope}>
      {() => {
        const row = read.rows[0];
        const s = { ...scope, row };
        return (
          <div className="record">
            {fields.length > 0 && <Fields fields={fields} row={row} scope={s} labels={labels} />}
            {groups.map((g) => (
              <details key={g.name} className="record-group" open={!g.collapsed}>
                <summary>{g.label ?? g.name}</summary>
                <Fields fields={g.fields} row={row} scope={s} labels={labels} />
              </details>
            ))}
            {Array.isArray(node.actions) && node.actions.length > 0 && (
              <div className="record-actions"><ActionBar actions={node.actions} scope={s} size="sm" handed={row} /></div>
            )}
          </div>
        );
      }}
    </ReadStates>
  );
}
