// The tests' `test`: Playwright's, failing a test whose page writes an error
// to the console or throws one uncaught. Built for development, React and
// React Aria report misuse (duplicate keys, say) only so.

import { test as base } from '@playwright/test'

export { expect } from '@playwright/test'
export type { Locator, Page } from '@playwright/test'

/** Errors that say nothing is wrong. */
const HARMLESS = [
  // The browser could not deliver every resize in one frame: it does so the
  // next.
  /ResizeObserver loop/,
  // The file worker blocked while a new version of the app takes over: the
  // client starts it again (src/workers/io-client.ts).
  /Cross-Origin-Embedder-Policy|due to access control checks/,
]

/** What a project in playwright.config.ts can ask of the tests. */
export interface Options {
  /**
   * WebGPU asked for (the chromium-webgpu project): the avatar must be drawn
   * and draped with it, not on the processor.
   */
  webgpu: boolean
}

export const test = base.extend<{ consoleErrors: string[] } & Options>({
  webgpu: [false, { option: true }],
  consoleErrors: [
    async ({ page }, use) => {
      const errors: string[] = []
      page.on('console', (m) => {
        if (m.type() === 'error') errors.push(m.text())
      })
      page.on('pageerror', (e) => errors.push(`uncaught: ${e.message}`))
      await use(errors)
      const real = errors.filter((e) => !HARMLESS.some((h) => h.test(e)))
      if (real.length > 0) {
        throw new Error(
          `The page reported errors:\n${real.map((e) => `  ${e.slice(0, 500)}`).join('\n')}`,
        )
      }
    },
    { auto: true },
  ],
})
