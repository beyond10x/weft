// The ratchet (ui-iteration-loop § 6): every key lower-is-better, the measured values of a run held
// against `ux/baseline.json`. It fails on a value worse than its baseline beyond the tolerance, on an
// improvement not written into the baseline in the same change, on a missing value (a key on one
// side only), and on a baseline entry whose value is not a number or whose tolerance is not the
// harness's own (`TOLERANCES`): the tolerance is never read from the file being checked.
//
//   node ux/ratchet.ts check  [--baseline <file>] [--measured <file>]   exit 1 on any violation
//   node ux/ratchet.ts update [--baseline <file>] [--measured <file>]   write the measured values in
//
// `--measured` defaults to `ux/.out/measured.json`, which the reporter (`ux/reporter.ts`) writes.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

export interface Entry {
  value: number;
  tolerance: number;
}

export interface Baseline {
  about: string;
  scenarios: Record<string, Record<string, Entry>>;
  tolerances: typeof TOLERANCES;
}

/** Per scenario (or `page.<name>`), per key, the measured value. */
export type Measured = Record<string, Record<string, number>>;

export interface Violation {
  kind: 'worse' | 'improved-not-taken' | 'missing' | 'bad-baseline';
  scenario: string;
  key: string;
  detail: string;
}

export const TOLERANCES = { counts: 0, layout_shift: 0.01 } as const;

export const tolerance = (key: string): number => (key === 'layout_shift' ? TOLERANCES.layout_shift : TOLERANCES.counts);

export const ABOUT =
  'UX ratchet of the documentation site (website/ux/). Scenarios: each screen-only user scenario (screen_only.actions, screen_only.looks, layout_shift: the worst over the gating viewports and themes) and each sitemap page as page.<name> (axe.moderate and axe.minor: violation nodes of that impact, the worse theme, at 1280x800). Every key is lower-is-better. Tolerances: counts 0, layout_shift 0.01. Write an improvement in with `node ux/ratchet.ts update` after a full run.';

const EPSILON = 1e-9;

export function check(baseline: Baseline, measured: Measured): Violation[] {
  const out: Violation[] = [];
  const scenarios = new Set([...Object.keys(baseline.scenarios), ...Object.keys(measured)]);
  for (const scenario of [...scenarios].sort()) {
    const base = baseline.scenarios[scenario] ?? {};
    const now = measured[scenario] ?? {};
    for (const key of [...new Set([...Object.keys(base), ...Object.keys(now)])].sort()) {
      const entry = base[key];
      const value = now[key];
      // The tolerance is the harness's own, never the one written in the file it checks.
      const allowed = tolerance(key);
      if (entry !== undefined && entry.tolerance !== allowed) {
        out.push({ kind: 'bad-baseline', scenario, key, detail: `tolerance ${entry.tolerance} in baseline.json, the harness's is ${allowed}` });
      }
      if (entry === undefined) {
        out.push({ kind: 'missing', scenario, key, detail: `no baseline value (measured ${value})` });
      } else if (typeof entry.value !== 'number' || !Number.isFinite(entry.value)) {
        out.push({ kind: 'bad-baseline', scenario, key, detail: `baseline value ${entry.value} is not a number` });
      } else if (value === undefined || value === null || !Number.isFinite(value)) {
        out.push({ kind: 'missing', scenario, key, detail: `no measured value (baseline ${entry.value})` });
      } else if (value > entry.value + allowed + EPSILON) {
        out.push({ kind: 'worse', scenario, key, detail: `${value} > ${entry.value} (tolerance ${allowed})` });
      } else if (value < entry.value - allowed - EPSILON) {
        out.push({ kind: 'improved-not-taken', scenario, key, detail: `${value} < ${entry.value}: write it into baseline.json in this change` });
      }
    }
  }
  return out;
}

/** The baseline that holds exactly the measured values. */
export function update(measured: Measured): Baseline {
  const scenarios: Baseline['scenarios'] = {};
  for (const scenario of Object.keys(measured).sort()) {
    const keys = measured[scenario]!;
    scenarios[scenario] = Object.fromEntries(
      Object.keys(keys)
        .sort()
        .map((key) => [key, { value: keys[key]!, tolerance: tolerance(key) }]),
    );
  }
  return { about: ABOUT, scenarios, tolerances: TOLERANCES };
}

export const report = (violations: readonly Violation[]): string[] => violations.map((v) => `ratchet: ${v.kind} ${v.scenario} ${v.key}: ${v.detail}`);

function main(argv: string[]): number {
  const ux = dirname(resolve(process.argv[1]!));
  const [command, ...rest] = argv;
  const option = (name: string, fallback: string) => {
    const at = rest.indexOf(name);
    return at >= 0 && rest[at + 1] ? resolve(rest[at + 1]!) : fallback;
  };
  const baselinePath = option('--baseline', join(ux, 'baseline.json'));
  const measuredPath = option('--measured', join(ux, '.out', 'measured.json'));
  if (command !== 'check' && command !== 'update') {
    console.error('usage: node ux/ratchet.ts check|update [--baseline <file>] [--measured <file>]');
    return 2;
  }
  if (!existsSync(measuredPath)) {
    console.error(`ratchet: ${measuredPath} is missing; run the harness first (npm --prefix website run ux)`);
    return 1;
  }
  const measured = JSON.parse(readFileSync(measuredPath, 'utf8')) as Measured;
  if (command === 'update') {
    writeFileSync(baselinePath, `${JSON.stringify(update(measured), null, 2)}\n`);
    console.log(`ratchet: wrote ${baselinePath}`);
    return 0;
  }
  const baseline: Baseline = existsSync(baselinePath) ? (JSON.parse(readFileSync(baselinePath, 'utf8')) as Baseline) : { about: ABOUT, scenarios: {}, tolerances: TOLERANCES };
  const violations = check(baseline, measured);
  for (const line of report(violations)) console.log(line);
  console.log(`ratchet: ${Object.keys(measured).length} scenarios measured, ${violations.length} violation(s)`);
  return violations.length ? 1 : 0;
}

if (/ratchet\.ts$/.test(process.argv[1] ?? '')) process.exit(main(process.argv.slice(2)));
