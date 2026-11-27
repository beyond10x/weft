// The run's own reporter: clears the measurements at the start, and at the end merges every test's
// measurements into `ux/.out/measured.json` (the worst value per key), holds them against
// `ux/baseline.json` (the ratchet, `ux/ratchet.ts`) and prints the ten slowest tests last. A ratchet
// violation fails the run. `UX_RATCHET=0` skips the ratchet for a partial run (`--grep`); a run that
// runs no test (`--list`) skips it too.

import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import type { FullResult, Reporter, TestCase, TestResult } from '@playwright/test/reporter';
import { OUT, UX } from './adapter.ts';
import { check, report } from './ratchet.ts';
import type { Baseline, Measured } from './ratchet.ts';

export const MEASURE = join(OUT, 'measure');

/** Merges measurement fragments: the worst (highest) value per key; a missing (null) value stays missing. */
export function merge(fragments: readonly Measured[]): Measured {
  const out: Measured = {};
  for (const fragment of fragments) {
    for (const [scenario, keys] of Object.entries(fragment)) {
      const into = (out[scenario] ??= {});
      for (const [key, value] of Object.entries(keys)) {
        const before = into[key];
        if (value === null || before === null) into[key] = null as unknown as number;
        else into[key] = before === undefined ? value : Math.max(before, value);
      }
    }
  }
  return out;
}

export default class UxReporter implements Reporter {
  private times: { title: string; ms: number }[] = [];
  private started = Date.now();

  onBegin(): void {
    rmSync(MEASURE, { recursive: true, force: true });
    mkdirSync(MEASURE, { recursive: true });
    this.started = Date.now();
  }

  onTestEnd(test: TestCase, result: TestResult): void {
    this.times.push({ title: test.titlePath().filter(Boolean).slice(1).join(' › '), ms: result.duration });
  }

  async onEnd(result: FullResult): Promise<{ status?: FullResult['status'] } | undefined> {
    let failed = false;
    // `playwright test --list` runs no test: nothing to measure or to time.
    if (this.times.length === 0) return undefined;
    if (process.env.UX_RATCHET !== '0' && result.status === 'passed') {
      const fragments = readdirSync(MEASURE)
        .filter((f) => f.endsWith('.json'))
        .sort()
        .map((f) => JSON.parse(readFileSync(join(MEASURE, f), 'utf8')) as Measured);
      const measured = merge(fragments);
      writeFileSync(join(OUT, 'measured.json'), `${JSON.stringify(measured, null, 2)}\n`);
      const baselinePath = join(UX, 'baseline.json');
      const baseline: Baseline = existsSync(baselinePath) ? (JSON.parse(readFileSync(baselinePath, 'utf8')) as Baseline) : { about: '', scenarios: {}, tolerances: { counts: 0, layout_shift: 0.01 } };
      const violations = check(baseline, measured);
      for (const line of report(violations)) console.log(line);
      console.log(`ratchet: ${Object.keys(measured).length} scenarios measured against ux/baseline.json, ${violations.length} violation(s)`);
      failed = violations.length > 0;
    } else {
      console.log(`ratchet: not run (${process.env.UX_RATCHET === '0' ? 'UX_RATCHET=0' : `the run ${result.status}`})`);
    }
    const slowest = [...this.times].sort((a, b) => b.ms - a.ms).slice(0, 10);
    console.log(`\nux: ${this.times.length} tests in ${((Date.now() - this.started) / 1000).toFixed(1)} s; the ten slowest:`);
    for (const t of slowest) console.log(`${(t.ms / 1000).toFixed(1).padStart(6)} s  ${t.title}`);
    return failed ? { status: 'failed' } : undefined;
  }
}
