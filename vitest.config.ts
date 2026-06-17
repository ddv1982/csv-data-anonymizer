import { defineConfig } from 'vitest/config';
import { resolve } from 'node:path';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src/renderer/src'),
      '@shared': resolve(__dirname, 'src/shared'),
    },
  },
  test: {
    include: ['tests/**/*.test.ts', 'src/renderer/src/**/*.test.ts'],
    environment: 'happy-dom',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      include: ['src/**/*.ts'],
      exclude: [
        'src/**/*.d.ts',
        'src/types/**/*.ts',
        'src/renderer/**/*.d.ts',
        'src/core/index.ts',
        'src/utils/index.ts',
      ],
    },
    globals: true,
  },
});
