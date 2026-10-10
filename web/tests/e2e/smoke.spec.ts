import { expect, test } from '@playwright/test'

test('app loads cross-origin isolated with the pattern engine', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'This device' })).toBeVisible()
  await expect(page.getByTestId('engine-status')).toHaveText(/Pattern engine \d+\.\d+\.\d+/)
  expect(await page.evaluate(() => globalThis.crossOriginIsolated)).toBe(true)
})

test('no request leaves the origin', async ({ page }) => {
  const foreign: string[] = []
  page.on('request', (req) => {
    if (new URL(req.url()).origin !== 'http://localhost:4173') foreign.push(req.url())
  })
  await page.goto('/')
  await expect(page.getByTestId('engine-status')).toHaveText(/Pattern engine/)
  expect(foreign).toEqual([])
})
