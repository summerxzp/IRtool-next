import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 30000,
  retries: 0,
  use: {
    baseURL: 'http://localhost:5173',
    headless: true,
    screenshot: 'only-on-failure',
    channel: 'chrome',
  },
  projects: [
    {
      name: 'chrome',
      use: { browserName: 'chromium', channel: 'chrome' },
    },
  ],
  webServer: {
    command: 'pnpm dev',
    port: 5173,
    reuseExistingServer: true,
  },
});
