import { expect, test } from '@playwright/test'

test('the avatar is fitted to the measurements and says how close it comes', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Measurements' }).click()
  const dialog = page.getByRole('dialog', { name: 'Measurements' })
  await dialog.getByRole('button', { name: 'Add my measurements' }).click()

  const fit = dialog.getByRole('button', { name: 'Fit the avatar' })
  await expect(fit).toBeDisabled()
  for (const [name, cm] of [
    ['Height', '165'],
    ['Bust', '91'],
    ['Waist', '71'],
    ['Hip', '97'],
  ] as const) {
    const field = dialog.getByRole('textbox', { name, exact: true })
    await field.fill(cm)
    await field.press('Tab')
  }
  // The age is kept with the measurements, for the avatar's shape.
  const age = dialog.getByRole('textbox', { name: 'Age' })
  await age.fill('52')
  await age.press('Tab')
  // The 3D side and the body file load with the first fit.
  await fit.click()
  await expect(
    dialog.getByText('The avatar matches all 4 measurements to within 1 cm.'),
  ).toBeVisible({ timeout: 60_000 })
  const report = dialog.getByRole('table', { name: 'How close the avatar comes' })
  await expect(report.getByRole('row')).toHaveCount(5)
  await expect(report.getByRole('row', { name: /^Waist 71\.0 cm/ })).toBeVisible()

  // Changed afterwards: the report says so until the avatar is fitted again.
  const waist = dialog.getByRole('textbox', { name: 'Waist', exact: true })
  await waist.fill('74')
  await waist.press('Tab')
  await expect(
    dialog.getByText('These measurements have changed since the avatar was fitted.'),
  ).toBeVisible()
  await dialog.getByRole('button', { name: 'Fit again' }).click()
  await expect(report.getByRole('row', { name: /^Waist 74\.0 cm/ })).toBeVisible({
    timeout: 60_000,
  })
  await expect(dialog.getByText(/changed since the avatar was fitted/)).toHaveCount(0)

  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(page.getByTestId('save-status')).toHaveText('Saved')
  await page.reload()
  await page.getByRole('button', { name: 'Measurements' }).click()
  await expect(
    page.getByRole('dialog', { name: 'Measurements' }).getByRole('textbox', { name: 'Age' }),
  ).toHaveValue('52')
})

test('the avatar fits offline once it has been fitted', async ({ page, context }, info) => {
  test.skip(info.project.name !== 'chromium', 'Playwright drives service workers in Chromium')
  await page.goto('/')
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null)
  await page.getByRole('button', { name: 'Measurements' }).click()
  let dialog = page.getByRole('dialog', { name: 'Measurements' })
  await dialog.getByRole('button', { name: 'Add my measurements' }).click()
  for (const [name, cm] of [
    ['Height', '170'],
    ['Bust', '96'],
    ['Waist', '78'],
    ['Hip', '102'],
  ] as const) {
    const field = dialog.getByRole('textbox', { name, exact: true })
    await field.fill(cm)
    await field.press('Tab')
  }
  // Offline before the first fit: the body file isn't here yet.
  await context.setOffline(true)
  await dialog.getByRole('button', { name: 'Fit the avatar' }).click()
  await expect(dialog.getByText(/hasn’t been downloaded yet/)).toBeVisible({ timeout: 60_000 })
  await context.setOffline(false)
  await dialog.getByRole('button', { name: 'Fit the avatar' }).click()
  await expect(dialog.getByText(/^The avatar matches/)).toBeVisible({ timeout: 60_000 })
  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(page.getByTestId('save-status')).toHaveText('Saved')

  // Offline, a fresh start: the body file comes from the service worker.
  await context.setOffline(true)
  await page.reload()
  await page.getByRole('button', { name: 'Measurements' }).click()
  dialog = page.getByRole('dialog', { name: 'Measurements' })
  await dialog.getByRole('button', { name: 'Fit the avatar' }).click()
  await expect(dialog.getByText(/^The avatar matches/)).toBeVisible({ timeout: 60_000 })
  await context.setOffline(false)
})
