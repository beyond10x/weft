// The shapes the renderer reads. An ess-ui/1 document is parsed from YAML and normalized by
// `normalize` (load.ts): page kinds merged, shorthands expanded. The types below name only what the
// renderer reads; every other key of the document passes through untouched.

/* eslint-disable @typescript-eslint/no-explicit-any */
export type Json = any;
export type Obj = Record<string, Json>;

export type Tone = 'neutral' | 'info' | 'success' | 'warning' | 'danger';

export interface Field extends Obj {
  field: string;
  name: string;
  label?: string;
  as?: string;
  unit?: 'ms' | 's' | 'm' | 'h';
  scale?: 'fraction' | 'hundredths';
  tone?: Tone;
  tone_by?: string;
  labels?: Record<string, string> | string;
  label_from?: { view: string; field: string; key?: string };
  sortable?: boolean;
  visible?: string;
  binds?: string;
  choice?: Obj;
  help?: { text?: string; link?: string };
  host?: string;
}

export interface Reads extends Obj {
  view?: string;
  placeholder?: string;
  fixture?: string;
  key?: string;
  params?: Record<string, string>;
  filter?: string;
  paging?: 'server' | 'client' | 'cursor' | 'append' | 'none';
  pager?: { page?: string; size?: string; sizes?: number[] };
}

export interface Action extends Obj {
  name: string;
  label?: string;
  does?: string;
  bind?: Record<string, string>;
  opens?: string;
  navigate?: { to: string; params?: Record<string, string> };
  export?: { reads: string; as: string; params?: Json };
  copy?: string;
  sets?: Record<string, string>;
  visible?: string;
  confirm?: string | { title: string; show?: string; confirm_label?: string };
  as?: string;
}

export interface Section extends Obj {
  name: string;
  component?: string;
  host?: string;
  title?: string;
  reads?: Reads;
  columns?: Field[];
  fields?: Field[];
  states?: Obj;
  state?: Record<string, StateDecl>;
}

export interface StateDecl extends Obj {
  type: Json;
  class: string;
  store?: string;
  default?: Json;
}

export interface Page extends Obj {
  name: string;
  kind: string;
  title?: string;
  params?: Record<string, Json>;
  actor?: string;
  header?: Obj;
  layout?: Json;
  state?: Record<string, StateDecl>;
  sections: Section[];
  overlays?: Record<string, Obj>;
  switch_to?: string[];
  nav?: { label?: string; synonyms?: string[] };
}

export interface Doc extends Obj {
  format: 'ess-ui/1';
  app: string;
  title?: string;
  model: string;
  shells: Record<string, Obj>;
  navigation: Obj;
  pages: Record<string, Page>;
  tokens?: Obj;
  themes?: Record<string, Obj>;
  theme?: { default: string; chosen_by?: string };
  tone_maps?: Record<string, Json>;
  label_maps?: Record<string, Record<string, string>>;
  widgets?: Record<string, Obj>;
}

export interface FixtureView {
  rows: Json[];
  total?: number;
}

export interface Fixtures {
  views: Record<string, FixtureView>;
  derived: Record<string, { by_id_from?: string; key?: string; same_as?: string }>;
}

export interface ModelInput {
  name: string;
  type: string;
  optional: boolean;
}

export interface ModelMeta {
  system: string;
  enums: Record<string, string[]>;
  commands: Record<string, { inputs: ModelInput[] }>;
  views: Record<string, { params: string[]; fields: string[] }>;
  /** Every actor of the model: its wire name (the role a principal holds), its grants, its attributes. */
  actors: Record<string, { wire: string; may: string[]; attributes: string[] }>;
}

export interface Bundle {
  doc: Doc;
  fixtures: Fixtures;
  actor: Obj;
  hosts: Record<string, Json>;
  model: ModelMeta;
  /** Other fixture actors (`actors/<name>.yaml` beside `actor.yaml`), chosen with `?actor=<name>`. */
  actors: Record<string, Obj>;
}
