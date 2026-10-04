import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['prototype/src/*.integration.test.jsx'],
    setupFiles: ['./prototype/src/ui-test-setup.js'],
    restoreMocks: true,
    // Inline OL so the COG adapter tests replace geotiff transport without
    // bypassing OpenLayers' actual asynchronous grid/view configuration.
    server: { deps: { inline: ['ol'] } },
  },
});
