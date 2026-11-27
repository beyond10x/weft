// The UX harness of the documentation site (`ux/`): layout, accessibility and screen-only user checks
// against the built site (`build/`), served on 127.0.0.1 by `ux/server.ts`. The site is built once,
// before the run (`npm run build`), and every test reads the same served build. `npm run ux` wraps
// the run in `timeout 120`.

import { defineConfig } from '@playwright/test';
import { BUILD, PORT } from './ux/adapter.ts';

export default defineConfig({
  testDir: './ux',
  testMatch: '*.spec.ts',
  outputDir: './ux/.out/test-results',
  fullyParallel: true,
  workers: 2,
  timeout: 60_000,
  forbidOnly: true,
  retries: 0,
  reporter: [['list'], ['./ux/reporter.ts']],
  use: {
    browserName: 'chromium',
    headless: true,
    deviceScaleFactor: 1,
    locale: 'en-US',
    timezoneId: 'UTC',
  },
  webServer: {
    command: `node ux/server.ts ${PORT} ${BUILD}`,
    url: `http://127.0.0.1:${PORT}/__ux/health`,
    reuseExistingServer: false,
    timeout: 30_000,
  },
});
