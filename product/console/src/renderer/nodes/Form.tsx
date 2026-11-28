// `form`: the fields a person fills for one command (`does`). Each control comes from the field's
// `as` and the command's input type in the model; an input the form does not show is taken from
// what the opener handed (a row, the page params), then from the signed-in actor, by name; an input
// the caller owns (account_id, tenant_id, the actor's attributes) from the actor first.

import { useId, useState } from 'react';
import type { FormEvent } from 'react';
import { useApp } from '../runtime.tsx';
import type { Scope } from '../expr.ts';
import { holds, values } from '../expr.ts';
import { findEnum } from '../load.ts';
import { callerOwned } from '../grants.ts';
import { useChoiceOptions } from './FilterBar.tsx';
import { fieldLabel } from './Collection.tsx';
import { Button } from '../../ui/kit.tsx';
import type { Field, Json, ModelInput, Obj } from '../types.ts';

type Draft = Record<string, Json>;

const blank = (v: Json) => v === undefined || v === null || v === '' || (Array.isArray(v) && v.length === 0);

/** The control a field gets: its `as`, else what the model's input type says. */
export function controlOf(f: Field, input: ModelInput | undefined, enumOf: (t: string) => string[] | undefined): string {
  if (f.as) return f.as;
  const t = input?.type ?? 'string';
  if (t === 'boolean') return 'toggle';
  if (t === 'integer' || t === 'decimal' || t === 'number') return 'number';
  if (t === 'timestamp') return 'timestamp';
  if (t === 'date') return 'date';
  if (t.startsWith('[')) return 'tags';
  if (enumOf(t)) return 'enum';
  return 'text';
}

const CALLER_OWNED = new Set(['account_id', 'tenant_id']);

/**
 * The input of a command: `bind` first, shown fields from the draft, the rest by name from what was
 * handed or the actor. An input the caller owns (`owned`: account_id, tenant_id and the attributes of
 * the actor) takes the actor's value over a handed row or param, so a row of another account cannot
 * send its account_id; an actor that carries no value for it leaves the handed one.
 */
export function commandInput(inputs: ModelInput[] | undefined, draft: Draft, shown: Set<string>, handed: Draft, actor: Obj, bound: Draft, owned: Set<string> = CALLER_OWNED): Draft {
  const out: Draft = { ...bound };
  if (!inputs) {
    for (const [k, v] of Object.entries(draft)) if (!blank(v)) out[k] = v;
    return out;
  }
  for (const i of inputs) {
    if (i.name in out) continue;
    const mine = owned.has(i.name) && !blank(actor?.[i.name]) ? actor[i.name] : undefined;
    const v = shown.has(i.name) ? draft[i.name] : mine ?? handed[i.name] ?? actor?.[i.name];
    if (!blank(v)) out[i.name] = v;
  }
  return out;
}

function parseValue(control: string, raw: string): Json {
  if (control === 'number') return raw === '' ? undefined : Number(raw);
  if (control === 'tags' || control === 'list') return raw.split(',').map((s) => s.trim()).filter(Boolean);
  if (control === 'timestamp') return raw ? new Date(`${raw}Z`).toISOString() : undefined;
  return raw;
}

function ChoiceControl({ f, id, value, onChange, scope, invalid, describedBy }: { f: Field; id: string; value: Json; onChange: (v: Json) => void; scope: Scope; invalid: boolean; describedBy?: string }) {
  const { options, status } = useChoiceOptions(f.choice, scope);
  const style = f.choice?.style;
  if (style === 'segmented' || style === 'radio') {
    return (
      <div className={style === 'segmented' ? 'segmented' : 'radios'} role="radiogroup" aria-labelledby={`${id}-label`} aria-invalid={invalid || undefined} aria-describedby={describedBy}>
        {options.map((o) => (
          <label key={o.value} className={`seg${value === o.value ? ' seg-on' : ''}`}>
            <input type="radio" name={id} value={o.value} checked={value === o.value} onChange={() => onChange(o.value)} />
            <span>{o.label}</span>
          </label>
        ))}
        {status === 'loading' && <span className="muted">Loading…</span>}
      </div>
    );
  }
  return (
    <select id={id} value={value ?? ''} onChange={(e) => onChange(e.target.value || undefined)} aria-invalid={invalid || undefined} aria-describedby={describedBy}>
      <option value="">{status === 'loading' ? 'Loading…' : status === 'idle' ? 'Fill the fields above first' : 'Choose…'}</option>
      {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  );
}

export function FormView({ node, scope, onDone }: { node: Obj; scope: Scope; onDone?: () => void }) {
  const app = useApp();
  const formId = useId();
  const inputs = node.does ? app.bundle.model.commands[node.does]?.inputs : undefined;
  const enumOf = (t: string) => findEnum(app.bundle.model, app.doc, t);
  const fields: Field[] = (node.fields ?? []).filter((f: Field) => holds(f.visible, scope));
  const handed: Draft = { ...(scope.params ?? {}), ...(scope.args ?? {}), ...(scope.row ?? {}) };
  const [draft, setDraft] = useState<Draft>(() => {
    const d: Draft = {};
    for (const f of fields) {
      if (f.default !== undefined) d[f.field] = f.default;
      else if (handed[f.field] !== undefined && typeof handed[f.field] !== 'object') d[f.field] = handed[f.field];
      else if (f.as === 'json' && handed[f.field] !== undefined) d[f.field] = JSON.stringify(handed[f.field], null, 2);
    }
    return d;
  });
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [tried, setTried] = useState(false);
  const input = (name: string) => inputs?.find((i) => i.name === name);
  const required = (f: Field) => f.required ?? (input(f.field) ? !input(f.field)!.optional : false);
  const set = (k: string, v: Json) => {
    setDraft((d) => ({ ...d, [k]: v }));
    if (tried) setErrors((e) => { const n = { ...e }; delete n[k]; return n; });
  };

  const validate = (): Record<string, string> => {
    const e: Record<string, string> = {};
    for (const f of fields) {
      const v = draft[f.field];
      if (required(f) && blank(v)) e[f.field] = `${fieldLabel(f)} is required.`;
      else if (f.as === 'json' && !blank(v)) {
        try { JSON.parse(String(v)); } catch { e[f.field] = 'Not valid JSON.'; }
      }
    }
    return e;
  };

  const submit = (ev: FormEvent) => {
    ev.preventDefault();
    setTried(true);
    const e = validate();
    setErrors(e);
    if (Object.keys(e).length) {
      document.getElementById(`${formId}-${Object.keys(e)[0]}`)?.focus();
      return;
    }
    const d: Draft = { ...draft };
    for (const f of fields) if (f.as === 'json' && !blank(d[f.field])) d[f.field] = JSON.parse(String(d[f.field]));
    const shown = new Set(fields.map((f) => f.field));
    const bound = values(node.bind, scope);
    if (node.does) app.runCommand(node.does, commandInput(inputs, d, shown, handed, app.actor, bound, callerOwned(app.bundle.model, app.actor)));
    onDone?.();
  };

  return (
    <form className="form" onSubmit={submit} noValidate aria-describedby={Object.keys(errors).length ? `${formId}-summary` : undefined}>
      {tried && Object.keys(errors).length > 0 && (
        <p id={`${formId}-summary`} className="form-summary" role="alert">Fix {Object.keys(errors).length === 1 ? 'one field' : `${Object.keys(errors).length} fields`} to continue.</p>
      )}
      {fields.map((f) => {
        const id = `${formId}-${f.field}`;
        const control = controlOf(f, input(f.field), enumOf);
        const err = errors[f.field];
        const helpId = f.help?.text ? `${id}-help` : undefined;
        const errId = err ? `${id}-err` : undefined;
        const describedBy = [helpId, errId].filter(Boolean).join(' ') || undefined;
        const v = draft[f.field];
        const req = required(f);
        const common = { id, 'aria-invalid': !!err || undefined, 'aria-describedby': describedBy, required: req };
        let el;
        if (control === 'choice' && f.choice) {
          el = <ChoiceControl f={f} id={id} value={v} onChange={(x) => set(f.field, x)} scope={{ ...scope, draft }} invalid={!!err} describedBy={describedBy} />;
        } else if (control === 'enum') {
          const opts = enumOf(input(f.field)!.type) ?? [];
          el = (
            <select {...common} value={v ?? ''} onChange={(e) => set(f.field, e.target.value || undefined)}>
              <option value="">Choose…</option>
              {opts.map((o) => <option key={o} value={o}>{o}</option>)}
            </select>
          );
        } else if (control === 'toggle') {
          el = <input {...common} type="checkbox" className="toggle" checked={!!v} onChange={(e) => set(f.field, e.target.checked)} />;
        } else if (control === 'json') {
          el = <textarea {...common} className="mono" rows={5} value={v ?? ''} placeholder="{ }" onChange={(e) => set(f.field, e.target.value)} />;
        } else if (control === 'text' && f.multiline) {
          el = <textarea {...common} rows={4} value={v ?? ''} onChange={(e) => set(f.field, e.target.value)} />;
        } else {
          const type = control === 'number' ? 'number' : control === 'timestamp' ? 'datetime-local' : control === 'date' ? 'date' : control === 'secret' ? 'password' : 'text';
          const shown = control === 'tags' || control === 'list' ? (Array.isArray(v) ? v.join(', ') : v ?? '') : control === 'timestamp' && v ? String(v).slice(0, 16) : v ?? '';
          el = (
            <input
              {...common}
              type={type}
              value={shown}
              autoComplete={control === 'secret' ? 'new-password' : 'off'}
              placeholder={control === 'tags' ? 'Comma separated' : f.placeholder}
              onChange={(e) => set(f.field, control === 'tags' || control === 'list' ? e.target.value.split(',').map((s) => s.trimStart()) : parseValue(control, e.target.value))}
              onBlur={control === 'tags' || control === 'list' ? (e) => set(f.field, parseValue(control, e.target.value)) : undefined}
            />
          );
        }
        return (
          <div key={f.field} className={`form-field${control === 'toggle' ? ' form-toggle' : ''}`}>
            <label htmlFor={id} id={`${id}-label`} className="form-label">
              {fieldLabel(f)}{req ? <span className="req" aria-hidden="true"> *</span> : <span className="opt"> optional</span>}
            </label>
            {el}
            {f.help?.text && <p id={helpId} className="form-help">{f.help.text}</p>}
            {err && <p id={errId} className="form-error">{err}</p>}
          </div>
        );
      })}
      <div className="form-actions">
        {onDone && <Button kind="ghost" onClick={onDone}>Cancel</Button>}
        <Button kind="primary" type="submit">{node.submit?.label ?? 'Save'}</Button>
      </div>
    </form>
  );
}
