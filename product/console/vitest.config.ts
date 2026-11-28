import { defineConfig } from 'vitest/config';

// Unit tests of the renderer: node only, no browser. A test over 60 s fails.
export default defineConfig({
  test: {
    include: ['test/**/*.test.ts'],
    environment: 'node',
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
