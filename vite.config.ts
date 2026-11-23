import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab · source viewable, not reusable: see LICENSE
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  build: { target: 'chrome110', sourcemap: false },
  test: { environment: 'node', include: ['src/**/*.test.ts'] },
});
