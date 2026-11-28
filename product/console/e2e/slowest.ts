// A Playwright reporter that prints the ten slowest tests and the run's wall time.

import type { FullResult, Reporter, TestCase, TestResult } from '@playwright/test/reporter';

export default class Slowest implements Reporter {
  private times: { title: string; ms: number }[] = [];
  private start = Date.now();
  onTestEnd(test: TestCase, result: TestResult) {
    this.times.push({ title: test.titlePath().slice(-1)[0], ms: result.duration });
  }
  onEnd(result: FullResult) {
    const top = [...this.times].sort((a, b) => b.ms - a.ms).slice(0, 10);
    console.log('\nslowest tests:');
    for (const t of top) console.log(`  ${(t.ms / 1000).toFixed(2).padStart(6)} s  ${t.title}`);
    console.log(`playwright ${result.status}: ${this.times.length} tests in ${((Date.now() - this.start) / 1000).toFixed(1)} s`);
  }
}
