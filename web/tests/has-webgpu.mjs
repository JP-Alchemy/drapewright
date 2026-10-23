// Whether Playwright's full Chromium, launched as the chromium-webgpu project
// launches it (playwright.config.ts), has a WebGPU adapter on this computer.
// Exits 0 if it has, 3 if not (any other code: the check itself failed).
// scripts/ci-local.sh asks before running the drape tests with WebGPU.

import { chromium } from '@playwright/test'

const browser = await chromium.launch({
  channel: 'chromium',
  args: ['--enable-unsafe-webgpu'],
})
try {
  const page = await browser.newPage()
  // WebGPU is offered only to secure pages: an https one, answered here.
  await page.route('https://webgpu.test/', (route) =>
    route.fulfill({ contentType: 'text/html', body: '<!doctype html>' }),
  )
  await page.goto('https://webgpu.test/')
  const adapter = await page.evaluate(async () => {
    const found = await navigator.gpu?.requestAdapter()
    return found ? `${found.info.vendor} ${found.info.architecture}`.trim() || 'yes' : null
  })
  console.log(adapter ? `WebGPU adapter: ${adapter}` : 'No WebGPU adapter in Chromium here.')
  process.exitCode = adapter ? 0 : 3
} finally {
  await browser.close()
}
