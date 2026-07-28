import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  retries: process.env.CI ? 2 : 0,
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  use: {
    baseURL: 'http://127.0.0.1:5173',
    trace: 'on-first-retry',
  },
  webServer: {
    command: process.env.CI
      ? 'npm run build && npm run preview -- --host 127.0.0.1'
      : 'npm run dev -- --host 127.0.0.1',
    url: 'http://127.0.0.1:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
})
