import { expect, test } from './fixtures.ts'

test('measurements are entered, checked and kept with the project', async ({ page }) => {
  await page.goto('/app/')
  await page.getByRole('button', { name: 'Measurements' }).click()
  const dialog = page.getByRole('dialog', { name: 'Measurements' })
  await dialog.getByRole('button', { name: 'Add my measurements' }).click()

  const bust = dialog.getByRole('textbox', { name: 'Bust', exact: true })
  await bust.fill('96')
  await bust.press('Tab')
  // A waist typed in millimetres while the project is in centimetres.
  const waist = dialog.getByRole('textbox', { name: 'Waist', exact: true })
  await waist.fill('780')
  await waist.press('Tab')
  await expect(dialog.getByText('Unusual for waist: check it is in the right units.')).toBeVisible()
  await waist.fill('78')
  await waist.press('Tab')
  await expect(dialog.getByText(/Unusual for waist/)).toHaveCount(0)

  // A man's figure says chest, and leaves out the bust's own measurements.
  await dialog.getByText('Man’s', { exact: true }).click()
  await expect(dialog.getByRole('textbox', { name: 'Chest', exact: true })).toHaveValue('96 cm')
  await expect(dialog.getByRole('textbox', { name: 'Underbust' })).toHaveCount(0)

  // Measured again: the first measurement is kept.
  await dialog.getByRole('button', { name: 'Measure again' }).click()
  await expect(dialog.getByText(/Measurement 2, taken .*1 earlier measurement kept/)).toBeVisible()

  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(page.getByTestId('save-status')).toHaveText('Saved')
  await page.reload()
  await page.getByRole('button', { name: 'Measurements' }).click()
  await expect(
    page
      .getByRole('dialog', { name: 'Measurements' })
      .getByRole('textbox', { name: 'Waist', exact: true }),
  ).toHaveValue('78 cm')
})
