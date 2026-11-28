// Binding expressions of ess-ui/1 (`Foundations › Expressions`): paths under a root (state, params,
// row, …), literals, `==`, `!=`, `in [..]`, `contains`, `not`, `and`, `or`, parentheses and the call
// forms `same_as(<section>)`, `matches(params)` and `actor.may(<command>)`. A string that does not
// parse, or reads no path, is literal text. A bare word that is no root (`Ready`) is a string.

import type { Json } from './types.ts';

export const ROOTS = ['state', 'params', 'args', 'row', 'selection', 'draft', 'section', 'channel', 'shell', 'url', 'actor', 'widget'] as const;

export type Expr =
  | { k: 'lit'; v: Json }
  | { k: 'path'; root: string; path: string[] }
  | { k: 'list'; items: Expr[] }
  | { k: 'not'; e: Expr }
  | { k: 'bin'; op: '==' | '!=' | 'in' | 'contains' | 'and' | 'or'; l: Expr; r: Expr }
  | { k: 'call'; fn: string; arg: string };

type Tok = { t: 'word' | 'num' | 'str' | 'op' | 'punct'; v: string };

function lex(src: string): Tok[] | null {
  const out: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) { i++; continue; }
    if (c === '"' || c === "'") {
      const end = src.indexOf(c, i + 1);
      if (end < 0) return null;
      out.push({ t: 'str', v: src.slice(i + 1, end) });
      i = end + 1;
      continue;
    }
    if (src.startsWith('==', i) || src.startsWith('!=', i)) { out.push({ t: 'op', v: src.slice(i, i + 2) }); i += 2; continue; }
    if ('[](),'.includes(c)) { out.push({ t: 'punct', v: c }); i++; continue; }
    const num = /^-?\d+(\.\d+)?(?![\w.])/.exec(src.slice(i));
    if (num) { out.push({ t: 'num', v: num[0] }); i += num[0].length; continue; }
    const word = /^[A-Za-z_][\w-]*(\.[A-Za-z_][\w-]*)*/.exec(src.slice(i));
    if (word) { out.push({ t: 'word', v: word[0] }); i += word[0].length; continue; }
    return null;
  }
  return out;
}

class Parser {
  i = 0;
  constructor(private toks: Tok[]) {}
  peek(): Tok | undefined { return this.toks[this.i]; }
  next(): Tok | undefined { return this.toks[this.i++]; }
  isWord(v: string): boolean { const t = this.peek(); return !!t && t.t === 'word' && t.v === v; }
  expect(v: string) { const t = this.next(); if (!t || t.v !== v) throw new Error(`expected ${v}`); }
  done(): boolean { return this.i >= this.toks.length; }

  or(): Expr {
    let l = this.and();
    while (this.isWord('or')) { this.next(); l = { k: 'bin', op: 'or', l, r: this.and() }; }
    return l;
  }
  and(): Expr {
    let l = this.not();
    while (this.isWord('and')) { this.next(); l = { k: 'bin', op: 'and', l, r: this.not() }; }
    return l;
  }
  not(): Expr {
    if (this.isWord('not')) { this.next(); return { k: 'not', e: this.not() }; }
    return this.cmp();
  }
  cmp(): Expr {
    const l = this.primary();
    const t = this.peek();
    if (t && t.t === 'op') { this.next(); return { k: 'bin', op: t.v as '==' | '!=', l, r: this.primary() }; }
    if (t && t.t === 'word' && (t.v === 'in' || t.v === 'contains')) { this.next(); return { k: 'bin', op: t.v, l, r: this.primary() }; }
    return l;
  }
  primary(): Expr {
    const t = this.next();
    if (!t) throw new Error('unexpected end');
    if (t.t === 'num') return { k: 'lit', v: Number(t.v) };
    if (t.t === 'str') return { k: 'lit', v: t.v };
    if (t.t === 'punct' && t.v === '(') { const e = this.or(); this.expect(')'); return e; }
    if (t.t === 'punct' && t.v === '[') {
      const items: Expr[] = [];
      while (!(this.peek()?.t === 'punct' && this.peek()?.v === ']')) {
        items.push(this.primary());
        if (this.peek()?.v === ',') this.next();
        else break;
      }
      this.expect(']');
      return { k: 'list', items };
    }
    if (t.t === 'word') {
      if (t.v === 'true') return { k: 'lit', v: true };
      if (t.v === 'false') return { k: 'lit', v: false };
      if (t.v === 'null') return { k: 'lit', v: null };
      if (this.peek()?.t === 'punct' && this.peek()?.v === '(') {
        this.next();
        const arg = this.next();
        this.expect(')');
        return { k: 'call', fn: t.v, arg: arg?.v ?? '' };
      }
      const [root, ...path] = t.v.split('.');
      if ((ROOTS as readonly string[]).includes(root)) return { k: 'path', root, path };
      return { k: 'lit', v: t.v };
    }
    throw new Error(`unexpected ${t.v}`);
  }
}

const cache = new Map<string, Expr | null>();

/** Parses an expression; `null` when the text is literal text (does not parse, or reads no path). */
export function parse(src: string): Expr | null {
  if (cache.has(src)) return cache.get(src)!;
  let out: Expr | null = null;
  const toks = lex(src);
  if (toks && toks.length) {
    try {
      const p = new Parser(toks);
      const e = p.or();
      if (p.done() && (readsPath(e) || toks.length === 1 || isOperatorExpr(e))) out = e;
    } catch {
      out = null;
    }
  }
  cache.set(src, out);
  return out;
}

function readsPath(e: Expr): boolean {
  switch (e.k) {
    case 'path': return true;
    case 'call': return true;
    case 'list': return e.items.some(readsPath);
    case 'not': return readsPath(e.e);
    case 'bin': return readsPath(e.l) || readsPath(e.r);
    default: return false;
  }
}

function isOperatorExpr(e: Expr): boolean {
  return e.k === 'bin' || e.k === 'not';
}

export interface Scope {
  state?: Json;
  params?: Json;
  args?: Json;
  row?: Json;
  selection?: Json;
  draft?: Json;
  shell?: Json;
  url?: Json;
  actor?: Json;
  widget?: Json;
  channel?: Json;
  /** `section.<name>.row` and `section.<name>.selection`. */
  section?: (name: string) => { row?: Json; selection?: Json } | undefined;
  /** `actor.may(<command>)`. */
  may?: (command: string) => boolean;
}

export function get(obj: Json, path: string[]): Json {
  let v = obj;
  for (const p of path) {
    if (v === null || v === undefined) return undefined;
    v = v[p];
  }
  return v;
}

function textOf(v: Json): string {
  if (v === null || v === undefined) return '';
  if (typeof v === 'object') return Object.values(v).map(textOf).join(' ');
  return String(v);
}

/** `<a> contains <b>`: `a` read as text holds every whitespace-separated word of `b`, in any case. */
export function containsWords(a: Json, b: Json): boolean {
  const words = textOf(b).toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  const hay = textOf(a).toLowerCase();
  return words.every((w) => hay.includes(w));
}

const same = (a: Json, b: Json): boolean => {
  const na = a === undefined ? null : a;
  const nb = b === undefined ? null : b;
  if (na === null || nb === null) return na === nb;
  if (typeof na === 'object' || typeof nb === 'object') return JSON.stringify(na) === JSON.stringify(nb);
  return String(na) === String(nb);
};

export function evaluate(e: Expr, s: Scope): Json {
  switch (e.k) {
    case 'lit': return e.v;
    case 'list': return e.items.map((i) => evaluate(i, s));
    case 'not': return !truthy(evaluate(e.e, s));
    case 'call':
      if (e.fn === 'may' || e.fn === 'actor.may') return s.may ? s.may(e.arg) : true;
      return undefined;
    case 'path': {
      if (e.root === 'section') {
        const [name, member, ...rest] = e.path;
        const sec = s.section?.(name);
        return get(member === 'selection' ? sec?.selection : sec?.row, rest);
      }
      if (e.root === 'actor' && e.path[0] === 'may') return undefined;
      return get((s as Json)[e.root], e.path);
    }
    case 'bin': {
      if (e.op === 'and') return truthy(evaluate(e.l, s)) && truthy(evaluate(e.r, s));
      if (e.op === 'or') return truthy(evaluate(e.l, s)) || truthy(evaluate(e.r, s));
      const l = evaluate(e.l, s);
      const r = evaluate(e.r, s);
      if (e.op === '==') return same(l, r);
      if (e.op === '!=') return !same(l, r);
      if (e.op === 'in') return Array.isArray(r) && r.some((x) => same(l, x));
      return containsWords(l, r);
    }
  }
}

export function truthy(v: Json): boolean {
  if (Array.isArray(v)) return v.length > 0;
  return v !== null && v !== undefined && v !== false && v !== '' && v !== 0;
}

/** Evaluates an `expr` value: a parsed expression, else the text itself. Non-strings pass through. */
export function value(src: Json, s: Scope): Json {
  if (typeof src !== 'string') return src;
  const e = parse(src);
  return e ? evaluate(e, s) : src;
}

/** A condition (`visible`, `filter`); absent means true. */
export function holds(src: string | undefined, s: Scope): boolean {
  if (src === undefined || src === null) return true;
  const e = parse(String(src));
  return e ? truthy(evaluate(e, s)) : true;
}

/** Evaluates every value of a map of expressions (read params, binds, navigate params). */
export function values(map: Record<string, string> | undefined, s: Scope): Record<string, Json> {
  const out: Record<string, Json> = {};
  for (const [k, v] of Object.entries(map ?? {})) out[k] = value(v, s);
  return out;
}

/** The roots and first segments an expression reads, for dependency tracking (`state.x` → `state.x`). */
export function reads(src: string): string[] {
  const e = parse(src);
  const out: string[] = [];
  const walk = (x: Expr) => {
    if (x.k === 'path') out.push([x.root, ...x.path].slice(0, 2).join('.'));
    else if (x.k === 'list') x.items.forEach(walk);
    else if (x.k === 'not') walk(x.e);
    else if (x.k === 'bin') { walk(x.l); walk(x.r); }
  };
  if (e) walk(e);
  return out;
}
