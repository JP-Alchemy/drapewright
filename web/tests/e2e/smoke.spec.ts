import { expect, test } from './fixtures.ts'

test('app loads cross-origin isolated with the pattern engine', async ({ page }) => {
  await page.goto('/app/')
  await expect(page.getByText('Open a sewing pattern to lay it out here.')).toBeVisible()
  expect(await page.evaluate(() => globalThis.crossOriginIsolated)).toBe(true)
  await page.getByRole('button', { name: 'About this device' }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('heading', { name: 'This device' })).toBeVisible()
  await expect(dialog.getByTestId('engine-status')).toHaveText(/Pattern engine \d+\.\d+\.\d+/)
})

test('no request leaves the origin', async ({ page, baseURL }) => {
  const origin = new URL(baseURL!).origin
  const foreign: string[] = []
  page.on('request', (req) => {
    if (new URL(req.url()).origin !== origin) foreign.push(req.url())
  })
  await page.goto('/app/')
  await expect(page.getByText('Open a sewing pattern to lay it out here.')).toBeVisible()
  expect(foreign).toEqual([])
})
