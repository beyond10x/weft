// Reads one console's ess-ui/1 document and everything it runs on: the fixtures its `fixtures:`
// index names, the fixture-mode actor (`actor.yaml`), the other actors a test may sign in as
// (`actors/*.yaml`, `?actor=<name>`) and host data (`hosts/*.yaml`) beside them,
// and the part of the ESS model it names (`ess specify compile`). The Vite plugin
// (`ess-ui-plugin.ts`) turns the result into a module; the unit tests call it directly.

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { basename, dirname, extname, join, resolve } from 'node:path';
import { parse } from 'yaml';
import { modelMeta, normalize } from '../src/renderer/load.ts';
import type { Bundle, Fixtures, Json, Obj } from '../src/renderer/types.ts';

export const CONSOLE = resolve(dirname(new URL(import.meta.url).pathname), '..');
export const UI = join(CONSOLE, 'ui');
export const ESS = resolve(CONSOLE, '..', '..', 'ess');
export const APPS = ['operator', 'account'] as const;

const yaml = (path: string): Json => parse(readFileSync(path, 'utf8'), { maxAliasCount: -1 });

let irCache: Obj | undefined;

/** The compiled ESS model (`ess specify compile --path ess --format json`), once per process. */
export function compileModel(essDir: string = ESS): Obj {
  if (!irCache) {
    const out = execFileSync('ess', ['specify', 'compile', '--path', essDir, '--format', 'json'], {
      maxBuffer: 256 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    irCache = JSON.parse(out.toString('utf8'));
  }
  return irCache!;
}

/** Fixtures of a document: its index, every view file it names, and the views it derives. */
export function loadFixtures(docPath: string, raw: Obj, files: string[]): Fixtures {
  const out: Fixtures = { views: {}, derived: {} };
  const index = raw.fixtures;
  if (!index) return out;
  const base = dirname(docPath);
  const dir = index.dir ? join(base, index.dir) : base;
  let spec: Obj = { ...index };
  if (index.index) {
    const at = join(base, index.index);
    files.push(at);
    spec = { ...spec, ...yaml(at) };
  }
  const read = new Map<string, Obj>();
  const file = (name: string): Obj => {
    const at = name.startsWith(index.dir ?? '\0') ? join(base, name) : join(dir, name);
    if (!read.has(at)) {
      files.push(at);
      read.set(at, yaml(at));
    }
    return read.get(at)!;
  };
  for (const [view, name] of Object.entries<string>(spec.views ?? {})) {
    const body = file(name);
    const entry = body.views ? body.views[view] : body.view === view ? body : undefined;
    if (!entry) throw new Error(`fixture ${name} holds no view ${view}`);
    out.views[view] = { rows: entry.rows ?? [], total: entry.total };
  }
  out.derived = spec.derived ?? {};
  return out;
}

/** The bundle of one console: normalized document, fixtures, fixture actor, host data and model. */
export function loadBundle(app: string, files: string[] = [], ir?: Obj): Bundle {
  const docPath = join(UI, `${app}.ui.yaml`);
  files.push(docPath);
  const raw = yaml(docPath);
  const doc = normalize(raw);
  const fixtures = loadFixtures(docPath, raw, files);
  const fxDir = raw.fixtures?.dir ? join(UI, raw.fixtures.dir) : UI;
  const actorPath = join(fxDir, 'actor.yaml');
  const actor = existsSync(actorPath) ? (files.push(actorPath), yaml(actorPath)) : {};
  const hosts = yamlDir(join(fxDir, 'hosts'), files);
  const actors = yamlDir(join(fxDir, 'actors'), files);
  const model = modelMeta(ir ?? compileModel(), raw);
  return { doc, fixtures, actor, actors, hosts, model };
}

/** Every `<name>.yaml` of a directory, keyed by name; empty when the directory does not exist. */
function yamlDir(dir: string, files: string[]): Record<string, Json> {
  const out: Record<string, Json> = {};
  if (!existsSync(dir)) return out;
  for (const f of readdirSync(dir).sort()) {
    if (extname(f) !== '.yaml') continue;
    files.push(join(dir, f));
    out[basename(f, '.yaml')] = yaml(join(dir, f));
  }
  return out;
}
