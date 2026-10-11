import { expect, test } from '@playwright/test'
import { fixture } from './helpers.ts'

test('opens an SVG pattern and lays out its pieces', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('inkscape-mm.svg'))

  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('heading', { name: 'Found 2 pieces' })).toBeVisible()
  await expect(dialog).toContainText('The file says how big it is')
  await expect(dialog).toContainText('Left out 1 small shape')
  // An SVG cannot say whether its outlines include seam allowance, so she is
  // asked, with nothing chosen for her.
  const add = dialog.getByRole('button', { name: 'Add 2 pieces' })
  await expect(add).toBeDisabled()
  const question = dialog.getByRole('radiogroup', {
    name: 'Is seam allowance included in these pieces?',
  })
  await expect(question.getByRole('radio', { checked: true })).toHaveCount(0)
  await question.getByText('Not included', { exact: true }).click()
  await add.click()

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
  await dialog.getByText('Not included', { exact: true }).click()
  await dialog.getByRole('button', { name: 'Add the piece' }).click()

  const list = page.getByRole('listbox', { name: 'Pieces' })
  await expect(list.getByRole('option', { name: /Pocket/ })).toContainText('7.5 cm × 3.7 cm')
})

test('opens a garment CAD (AAMA) DXF file', async ({ page }) => {
  await page.goto('/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('aama-skirt.dxf'))

  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('heading', { name: 'Found 2 pieces' })).toBeVisible()
  // METRIC is read as millimetres, but she confirms it.
  await expect(dialog).toContainText('which usually means millimetres')
  await expect(dialog.getByRole('radio', { name: /Millimetres/ })).toBeChecked()
  await expect(dialog).toContainText('Skirt front would be 25.0 cm × 60.0 cm')
  // The skirt's allowance was measured from its two lines; only the
  // waistband, drawn with its cutting line alone, is asked about.
  await expect(dialog).toContainText(
    'Skirt front: found both the cutting line and the stitching line',
  )
  const question = dialog.getByRole('radiogroup', {
    name: 'Is seam allowance included in “Waistband”?',
  })
  await expect(question).toContainText('The file marks this outline as the cutting line')
  await expect(question.getByRole('radio', { name: /Already included/ })).toBeChecked()
  const width = dialog.getByRole('textbox', { name: 'Seam allowance' })
  await width.fill('1')
  await width.press('Tab')
  await dialog.getByRole('button', { name: 'Add 2 pieces' }).click()

  const list = page.getByRole('listbox', { name: 'Pieces' })
  await expect(list.getByRole('option', { name: /Waistband/ })).toContainText('38.0 cm × 8.0 cm')
  await expect(list.getByRole('option', { name: /Skirt front/ })).toContainText('25.0 cm × 60.0 cm')
  const allowances = await page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.engine.document().pieces.map((p: any) => [p.name, p.allowance]),
  )
  expect(allowances).toEqual([
    ['Skirt front', { mode: 'add', seam: 15, hem: null }],
    ['Waistband', { mode: 'included', seam: 10, hem: null }],
  ])
})
