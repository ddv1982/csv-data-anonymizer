import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['tests/**/*.test.ts'],
    environment: 'node',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov', 'html'],
      include: ['src/**/*.ts'],
      exclude: [
        'src/**/*.d.ts',
        'src/types/**/*.ts',
        // CLI entry points and command files are tested via subprocess in CLI integration tests
        // Coverage cannot be captured for subprocess execution
        'src/index.ts',
        'src/cli/commands/**/*.ts',
        // CLI prompts are interactive and tested through integration tests
        'src/cli/prompts/**/*.ts',
        'src/cli/output/index.ts',
        'src/config/index.ts',
        'src/core/index.ts',
        'src/utils/index.ts',
      ],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 80,
        statements: 80,
      },
    },
    globals: true,
  },
});
