import { resolve } from 'node:path'
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: resolve(process.cwd(), 'tests/e2e'),
  globalSetup: resolve(process.cwd(), 'tests/e2e/global-setup.ts'),
  outputDir: 'test-results',
  fullyParallel: false,
  workers: 1,
  timeout: 90_000,
  expect: { timeout: 15_000 },
  reporter: [
    ['list'],
    ...(process.env.GITHUB_ACTIONS ? [['github'] as const] : []),
    ['html', { outputFolder: 'playwright-report', open: 'never' }],
  ],
  use: {
    baseURL: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:3000',
    browserName: 'chromium',
    headless: true,
    viewport: { width: 375, height: 812 },
    actionTimeout: 20_000,
    navigationTimeout: 45_000,
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
})
