import { expect, type Page, test } from './fixtures.ts'
import { fixture, modKey, openWithFixture, placementOf } from './helpers.ts'

// Several imports and turns each: slow on the iPad emulation on a small runner.
test.describe.configure({ timeout: 60_000 })

/** Each piece's grain on the table, degrees from pointing right, and its placement. */
const grains = (page: Page) =>
  page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.engine
      .document()
      .pieces.map(
        (p: {
          name: string
          placement: { x: number; y: number; rotation: number }
          grainline: { from: [number, number]; to: [number, number] } | null
        }) => {
          const g = p.grainline
          const along = g
            ? (Math.atan2(g.to[1] - g.from[1], g.to[0] - g.from[0]) * 180) / Math.PI +
              p.placement.rotation
            : null
          return { name: p.name, placement: p.placement, along }
        },
      ),
  )

/** Up and down the table, either way: 90° from pointing right, give or take half a turn. */
const upright = (along: number) =>
  Math.abs((((along - 90) % 180) + 180) % 180) < 0.01 ||
  Math.abs((((along - 90) % 180) + 180) % 180) > 179.99

/** Opens the pattern drawn 3° askew in sizes 8, 10 and 12, straightened or not. */
async function openAskew(page: Page, straighten: boolean) {
  await page.goto('/app/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('sizes-askew.pdf'))
  const dialog = page.getByRole('dialog')
  for (const size of ['8', '10', '12']) {
    const chip = dialog.getByRole('button', { name: new RegExp(`^${size}\\b`) })
    if ((await chip.getAttribute('aria-pressed')) !== 'true') await chip.click()
  }
  await dialog.getByText('Already included', { exact: true }).click()
  const choice = dialog.getByRole('switch', { name: 'Straighten pieces to their grainline' })
  // Offered, and on, for pieces drawn askew.
  await expect(choice).toBeChecked()
  await expect(dialog.getByText(/All 6 pieces are drawn a little askew/)).toBeVisible()
  if (!straighten) await dialog.getByText('Straighten pieces to their grainline').click()
  await expect(choice).toBeChecked({ checked: straighten })
  await dialog.getByRole('button', { name: 'Add 6 pieces' }).click()
  await expect(dialog).toHaveCount(0)
}

test('a pattern drawn askew is straightened to its grainlines as it comes in', async ({ page }) => {
  await openAskew(page, true)
  const pieces = await grains(page)
  expect(pieces).toHaveLength(6)
  for (const p of pieces) {
    expect(upright(p.along!), p.name).toBe(true)
    expect(p.placement.rotation).toBeCloseTo(357)
  }
  // Each pattern piece's sizes still nested as printed: placed alike.
  const fronts = pieces.filter((p: { name: string }) => p.name.startsWith('Front bodice'))
  for (const f of fronts) {
    expect(f.placement.x).toBeCloseTo(fronts[0].placement.x, 6)
    expect(f.placement.y).toBeCloseTo(fronts[0].placement.y, 6)
  }
})

test('pieces chosen together straighten to their grainlines as one undo step', async ({ page }) => {
  await openAskew(page, false)
  let pieces = await grains(page)
  for (const p of pieces) {
    expect(p.placement.rotation, p.name).toBe(0)
    expect(upright(p.along!), p.name).toBe(false)
  }
  await page.locator('canvas').focus()
  await page.keyboard.press(`${await modKey(page)}+a`)
  await expect(page.getByRole('heading', { name: '6 pieces' })).toBeVisible()
  await page.getByRole('button', { name: 'Straighten to the grainline' }).click()
  await expect(page.getByRole('status').filter({ hasText: 'Straightened 6 pieces' })).toBeVisible()
  pieces = await grains(page)
  for (const p of pieces) expect(upright(p.along!), p.name).toBe(true)
  // The sizes turned together: still nested.
  const sleeves = pieces.filter((p: { name: string }) => p.name.startsWith('Sleeve'))
  for (const s of sleeves) {
    expect(s.placement.x).toBeCloseTo(sleeves[0].placement.x, 6)
    expect(s.placement.y).toBeCloseTo(sleeves[0].placement.y, 6)
  }
  await expect(page.getByRole('textbox', { name: 'Turned (degrees clockwise)' })).toHaveValue('-3')

  // Again: already straight.
  await page.getByRole('button', { name: 'Straighten to the grainline' }).click()
  await expect(
    page.getByRole('status').filter({ hasText: 'Already straight along their grainlines' }),
  ).toBeVisible()

  // One undo puts all six back.
  await page.getByRole('button', { name: 'Undo turn piece' }).click()
  for (const p of await grains(page)) expect(p.placement.rotation, p.name).toBe(0)
})

test('a piece turns to any angle typed in, and says when it has no grainline', async ({ page }) => {
  await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
  await page.getByRole('row', { name: /Skirt front/, level: 1 }).click()
  const angle = page.getByRole('textbox', { name: 'Turned (degrees clockwise)' })
  await expect(angle).toHaveValue('0')
  await angle.fill('12.5')
  await angle.press('Enter')
  await expect.poll(async () => (await placementOf(page, 0)).rotation).toBeCloseTo(12.5)
  // Only where it lies: the stepper turns it on a degree at a time.
  await page.getByRole('button', { name: 'Increase Turned (degrees clockwise)' }).click()
  await expect.poll(async () => (await placementOf(page, 0)).rotation).toBeCloseTo(13.5)
  await page.getByRole('button', { name: 'Straighten to the grainline' }).click()
  await expect.poll(async () => (await placementOf(page, 0)).rotation).toBeCloseTo(0)

  // The waistband has no grainline: it says so, and stays as it is.
  await page.getByRole('row', { name: /Waistband/, level: 1 }).click()
  await angle.fill('5')
  await angle.press('Enter')
  await page.getByRole('button', { name: 'Straighten to the grainline' }).click()
  await expect(
    page.getByRole('status').filter({ hasText: '“Waistband” has no grainline to straighten to' }),
  ).toBeVisible()
  expect((await placementOf(page, 1)).rotation).toBeCloseTo(5)
})
