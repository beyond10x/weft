// Style tokens (`Tokens`, `themes`, `theme`) to CSS custom properties. The built-in table is merged
// under the document's `tokens:`, and each theme is that base with its own overrides, so every
// theme defines every token. A theme named `system` follows the reader's `prefers-color-scheme`
// between the themes named `light` and `dark` (a convention of this renderer: ess-ui/1 has no form
// for "follow the system").

import type { Doc, Obj } from './types.ts';

export const BUILTIN_TOKENS: Obj = {
  color: {
    bg: '#f7f7f8', surface: '#ffffff', text: '#1c1d21', muted: '#676a73', line: '#dcdde1', accent: '#2f5bd3',
    info: '#2f7fd3', success: '#2e8a4f', warning: '#b7791f', danger: '#c0392b', danger_fill: '#f6d5d1',
    success_fill: '#d7f0df', warning_fill: '#f7e7c6', info_fill: '#d9e4fb', focus: '#eef2fd', on_accent: '#fff',
    backdrop: 'rgb(0 0 0 / 0.3)',
  },
  space: { xs: '0.25rem', sm: '0.5rem', md: '1rem', lg: '1.5rem' },
  radius: { sm: '4px', md: '6px', lg: '8px', pill: '999px' },
  type: {
    body: { family: 'system-ui, sans-serif' },
    caption: { size: '0.8rem' },
    heading: { size: '1.05rem', weight: 600 },
    mono: { family: 'ui-monospace, monospace' },
  },
  tone: {
    neutral: { text: 'text', fill: 'line' },
    info: { text: 'info', fill: 'line' },
    success: { text: 'success', fill: 'line' },
    warning: { text: 'warning', fill: 'line' },
    danger: { text: 'danger', fill: 'danger_fill' },
  },
};

function mergeGroups(base: Obj, over: Obj | undefined): Obj {
  const out: Obj = {};
  for (const g of ['color', 'space', 'radius', 'type', 'tone']) {
    out[g] = { ...(base[g] ?? {}) };
    for (const [k, v] of Object.entries<Obj>(over?.[g] ?? {})) {
      out[g][k] = g === 'type' ? { ...v } : g === 'tone' ? { ...(out[g][k] ?? {}), ...v } : v;
    }
  }
  return out;
}

/** The full token table of every theme of the document (`{}` themes: the base). */
export function themeTokens(doc: Doc): Record<string, Obj> {
  const base = mergeGroups(BUILTIN_TOKENS, doc.tokens);
  const themes = doc.themes ?? { light: {} };
  const out: Record<string, Obj> = {};
  for (const [name, over] of Object.entries(themes)) out[name] = mergeGroups(base, over);
  return out;
}

/** CSS custom properties of one token table. */
export function cssVars(t: Obj): string {
  const lines: string[] = [];
  for (const [k, v] of Object.entries<string>(t.color)) lines.push(`--color-${k}: ${v};`);
  for (const [k, v] of Object.entries<string>(t.space)) lines.push(`--space-${k}: ${v};`);
  for (const [k, v] of Object.entries<string>(t.radius)) lines.push(`--radius-${k}: ${v};`);
  const body = t.type.body ?? {};
  for (const style of ['body', 'caption', 'heading', 'mono']) {
    const s = { ...body, ...(t.type[style] ?? {}) };
    if (s.family) lines.push(`--type-${style}-family: ${s.family};`);
    if (s.size) lines.push(`--type-${style}-size: ${s.size};`);
    if (s.weight) lines.push(`--type-${style}-weight: ${s.weight};`);
  }
  for (const [tone, v] of Object.entries<Obj>(t.tone)) {
    lines.push(`--tone-${tone}-text: var(--color-${v.text});`);
    lines.push(`--tone-${tone}-fill: var(--color-${v.fill});`);
  }
  return lines.join('\n  ');
}

/** The stylesheet of every theme, keyed by `[data-theme=<name>]` on the root element. */
export function themeCss(doc: Doc): string {
  const all = themeTokens(doc);
  const parts: string[] = [];
  for (const [name, t] of Object.entries(all)) {
    if (name === 'system') continue;
    parts.push(`:root[data-theme="${name}"] {\n  ${cssVars(t)}\n  color-scheme: ${name === 'dark' ? 'dark' : 'light'};\n}`);
  }
  return parts.join('\n');
}

/** The theme to draw for a chosen theme name (`system` resolves by the color scheme). */
export function resolveTheme(doc: Doc, chosen: string | undefined, prefersDark: boolean): string {
  const themes = doc.themes ?? { light: {} };
  const name = chosen && chosen in themes ? chosen : doc.theme?.default ?? Object.keys(themes)[0];
  if (name === 'system') return prefersDark && 'dark' in themes ? 'dark' : 'light' in themes ? 'light' : Object.keys(themes)[0];
  return name;
}
