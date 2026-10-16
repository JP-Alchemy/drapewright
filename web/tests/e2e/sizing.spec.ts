import { expect, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

/** Opens the graded skirt for Jo, its allowance answered, and measures every size. */
async function measured(page: import('@playwright/test').Page) {
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
  return { dialog, finder, table }
}

test('every size is sewn and measured on someone, and one is chosen', async ({ page }) => {
  test.setTimeout(120_000)
  const { dialog, finder, table } = await measured(page)
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

test('a size at the waist and another at the hip, blended between', async ({ page }) => {
  test.setTimeout(120_000)
  const { dialog, finder } = await measured(page)
  const cut = finder.getByRole('group', { name: 'Size to cut at each' })
  for (const [level, size] of [
    ['Waist', '36'],
    ['Hip', '42'],
  ] as const) {
    await cut.getByRole('button', { name: new RegExp(level) }).click()
    await page.getByRole('option', { name: size, exact: true }).click()
  }
  await cut.getByRole('button', { name: 'Blend between them' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(
    page.getByText('2 pieces blended between sizes: 36 at the waist and 42 at the hip.'),
  ).toBeVisible()
  const list = page.getByRole('treegrid', { name: 'Pieces' })
  await expect(list.getByRole('row', { level: 1 })).toHaveText([/^Skirt front/, /^Skirt back/])
  // The front: size 36's waist (170 mm), size 42's hem (285 mm).
  const widths = await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const front = s.engine.document().pieces.find((p: { name: string }) => p.name === 'Skirt front')
    const points: [number, number][] = [
      front.outline.start,
      ...front.outline.edges.flatMap((e: { segments: { to: [number, number] }[] }) =>
        e.segments.map((g) => g.to),
      ),
    ]
    const xs = points.map((p) => p[0])
    const top = Math.min(...points.map((p) => p[1]))
    const atTop = points.filter((p) => p[1] < top + 0.5).map((p) => p[0])
    return {
      waist: Math.max(...atTop) - Math.min(...atTop),
      hem: Math.max(...xs) - Math.min(...xs),
      notes: front.notes,
    }
  })
  expect(Math.abs(widths.waist - 170), `waist ${widths.waist}`).toBeLessThan(3)
  expect(Math.abs(widths.hem - 285), `hem ${widths.hem}`).toBeLessThan(1)
  expect(widths.notes).toBe('Blended between sizes: 36 at the waist and 42 at the hip.')
})
