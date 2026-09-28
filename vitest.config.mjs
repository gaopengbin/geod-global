import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['prototype/src/*.integration.test.jsx'],
    setupFiles: ['./prototype/src/ui-test-setup.js'],
    restoreMocks: true,
  },
});
