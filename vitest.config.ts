import { defineConfig } from 'vitest/config'

// Unit tests live next to the code as `*.test.ts(x)`. Playwright specs in
// web/tests/e2e are run by `pnpm e2e`, not here.
export default defineConfig({
  test: {
    include: ['packages/*/src/**/*.test.ts', 'web/src/**/*.test.{ts,tsx}'],
    environment: 'node',
    passWithNoTests: true,
  },
})
