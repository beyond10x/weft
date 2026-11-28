import { defineConfig, devices } from '@playwright/test';

// One port per run, chosen once in the main process; workers inherit it through the environment.
process.env.CONSOLE_E2E_PORT ??= String(20000 + Math.floor(Math.random() * 20000));
const port = Number(process.env.CONSOLE_E2E_PORT);

export default defineConfig({
  testDir: 'e2e',
  testMatch: 'consoles.spec.ts',
  timeout: 60_000,
  globalTimeout: 100_000,
  expect: { timeout: 10_000 },
  fullyParallel: true,
  workers: 2,
  retries: 0,
  reporter: [['list'], ['./e2e/slowest.ts']],
  outputDir: '../../target/console/test-results',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    ...devices['Desktop Chrome'],
    reducedMotion: 'reduce',
    timezoneId: 'UTC',
    locale: 'en-GB',
  },
  webServer: {
    command: `node e2e/server.mjs ${port} ../../target/console/dist`,
    url: `http://127.0.0.1:${port}/`,
    reuseExistingServer: false,
    timeout: 15_000,
  },
});
