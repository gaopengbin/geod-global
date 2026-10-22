import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['prototype/src/*.integration.test.jsx'],
    setupFiles: ['./prototype/src/ui-test-setup.js'],
    restoreMocks: true,
    // GIS/Agent suites each build a full DOM and map module graph. Unbounded
    // workers starve timers and leak timed-out interactions into later tests.
    maxWorkers: 2,
    // Complete user journeys render multiple large GIS/Agent views. On the
    // hosted Windows runner the combined work can exceed five seconds even
    // when every bounded interaction succeeds. Retain finite per-case limits
    // and all assertion/waitFor deadlines; this is not a UI latency benchmark.
    testTimeout: process.env.CI && process.platform === 'win32' ? 15_000 : 5_000,
    // Inline OL so the COG adapter tests replace geotiff transport without
    // bypassing OpenLayers' actual asynchronous grid/view configuration.
    server: { deps: { inline: ['ol'] } },
  },
});
