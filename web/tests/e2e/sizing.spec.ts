import { expect, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

test('every size is sewn and measured on someone, and one is chosen', async ({ page }) => {
  test.setTimeout(120_000)
  await page.goto('/app/?debug')
  await expect(page.getByText('Open a sewing pattern to lay it out here.')).toBeVisible()
  // Someone's measurements (the debug handle; the Measurements dialog does
  // the same).
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    s.dispatch({
      type: 'addMeasurementSet',
      set: {
        id: '',
        name: 'Jo',
        figure: 'woman',
        revision: 1,
        takenOn: '2026-10-13',
        age: null,
        values: { height: 1650, bust: 880, waist: 700, hip: 960 },
        previous: [],
      },
    })
  })
  // A skirt in four sizes, 36 to 42, its allowance not included.
  await page.getByTestId('pattern-file').setInputFiles(fixture('aama-graded-skirt.dxf'))
  const dialog = page.getByRole('dialog')
  await dialog
    .getByRole('radiogroup', { name: /seam allowance included/ })
    .getByText('Not included', { exact: true })
    .click()
  const finder = dialog.getByRole('region', { name: 'Which size fits?' })
  await finder.getByRole('button', { name: 'Sew and measure every size on Jo' }).click()
  const table = finder.getByRole('table')
  await expect(table.getByRole('row')).toHaveCount(5, { timeout: 90_000 })
  // Each size measured round the waist, 4 cm bigger than the one before.
  // (A skirt reaches no bust: its columns are the waist and the hip.)
  await expect(table.getByRole('columnheader')).toHaveText(['Size', 'Waist', 'Hip', 'Choose'])
  const waists = await table
    .getByRole('row')
    .evaluateAll((rows) => rows.slice(1).map((r) => r.querySelectorAll('td')[0]?.textContent ?? ''))
  const cm = waists.map((w) => Number.parseFloat(w))
  expect(cm.every(Number.isFinite), waists.join(' | ')).toBe(true)
  for (let i = 1; i < cm.length; i++) expect(cm[i]! - cm[i - 1]!).toBeCloseTo(4, 0)
  await expect(finder.getByRole('status')).toHaveText(/^For Jo: /)
  // Chosen from the table: that size is the one added.
  await table.getByRole('button', { name: 'Choose 42' }).click()
  await expect(dialog.getByRole('button', { name: '42', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  )
  await expect(dialog.getByRole('button', { name: /^38/ })).toHaveAttribute('aria-pressed', 'false')
})
