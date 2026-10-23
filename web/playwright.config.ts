import { defineConfig, devices } from '@playwright/test'
import type { Options } from './tests/e2e/fixtures.ts'

// End-to-end tests run against the production build (service worker,
// headers and all), in Chromium and in WebKit emulating an iPad with touch;
// and in Chromium against the same app built for development, where React
// and React Aria check what they are given (duplicate keys and the like) and
// say so in the console, which fails the test (tests/e2e/fixtures.ts).
//
// With E2E_WEBGPU=1, also the drape tests in full Chromium with WebGPU, which
// draws the avatar and drapes on the graphics card as most seamstresses'
// browsers do. The headless shell above has only WebGL2, so drapes on the
// processor; CI's runners have no graphics card. scripts/ci-local.sh runs
// it where there is one.
export default defineConfig<Options>({
  testDir: 'tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: 'http://localhost:4173',
    trace: 'on-first-retry',
  },
  webServer: [
    {
      command: 'pnpm exec vite build && pnpm exec vite preview',
      url: 'http://localhost:4173',
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
    {
      command:
        'NODE_ENV=development pnpm exec vite build --mode development --outDir dist-dev && pnpm exec vite preview --outDir dist-dev --port 4174',
      url: 'http://localhost:4174',
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
  ],
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'ipad-webkit', use: { ...devices['iPad Pro 11'] } },
    {
      name: 'chromium-dev',
      use: { ...devices['Desktop Chrome'], baseURL: 'http://localhost:4174' },
    },
    ...(process.env.E2E_WEBGPU === '1'
      ? [
          {
            name: 'chromium-webgpu',
            testMatch: 'drape.spec.ts',
            use: {
              ...devices['Desktop Chrome'],
              channel: 'chromium',
              launchOptions: { args: ['--enable-unsafe-webgpu'] },
              webgpu: true,
            },
          },
        ]
      : []),
  ],
})
