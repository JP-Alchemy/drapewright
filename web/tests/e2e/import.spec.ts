import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'

const fixture = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/svg/${name}`, import.meta.url))

test('opens an SVG pattern and lays out its pieces', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('inkscape-mm.svg'))

  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('heading', { name: 'Found 2 pieces' })).toBeVisible()
  await expect(dialog).toContainText('The file says how big it is')
  await expect(dialog).toContainText('Left out 1 small shape')
  await dialog.getByRole('button', { name: 'Add 2 pieces' }).click()

  const list = page.getByRole('listbox', { name: 'Pieces' })
  await expect(list.getByRole('option', { name: /Skirt front/ })).toBeVisible()
  // The waistband is drawn 80 × 700 mm in the file.
  await expect(list.getByRole('option', { name: /Waistband/ })).toContainText('8.0 cm × 70.0 cm')

  await page.getByRole('button', { name: 'Undo add pieces' }).click()
  await expect(page.getByText('No pieces yet.')).toBeVisible()
  await page.getByRole('button', { name: 'Redo add pieces' }).click()
  await expect(list.getByRole('option')).toHaveCount(2)
})

test('asks how big a file without units is', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('illustrator-pt.svg'))

  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('radiogroup', { name: 'How big is this pattern?' })).toBeVisible()
  await expect(dialog).toContainText('Pocket would be 10.0 cm × 5.0 cm')
  await dialog.getByText('96 to the inch (most apps)').click()
  await expect(dialog.getByRole('radio', { name: /96 to the inch/ })).toBeChecked()
  await dialog.getByRole('button', { name: 'Add the piece' }).click()

  const list = page.getByRole('listbox', { name: 'Pieces' })
  await expect(list.getByRole('option', { name: /Pocket/ })).toContainText('7.5 cm × 3.7 cm')
})
