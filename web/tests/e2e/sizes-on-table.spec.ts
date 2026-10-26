import { expect, type Page, test } from './fixtures.ts'
import { edgeMiddle, fixture } from './helpers.ts'

/** The darkest of the canvas's pixels within 3 px of a point on the page, 0 to 255. */
const darkestNear = (page: Page, at: { x: number; y: number }) =>
  page.evaluate((at) => {
    const canvas = document.querySelector('canvas')!
    const r = canvas.getBoundingClientRect()
    const k = canvas.width / r.width
    const [x, y] = [Math.round((at.x - r.left) * k), Math.round((at.y - r.top) * k)]
    const reach = Math.ceil(3 * k)
    const data = canvas
      .getContext('2d')!
      .getImageData(x - reach, y - reach, 2 * reach + 1, 2 * reach + 1).data
    let darkest = 255
    for (let i = 0; i < data.length; i += 4) {
      darkest = Math.min(darkest, (data[i]! + data[i + 1]! + data[i + 2]!) / 3)
    }
    return darkest
  }, at)

test('a pattern’s sizes laid over each other show all their lines', async ({ page }) => {
  // Sizes 8, 10 and 12, nested as printed: each size's pieces where the
  // file draws them, the smallest under the others.
  await page.goto('/app/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('sizes-layers.pdf'))
  const dialog = page.getByRole('dialog')
  for (const size of ['8', '10', '12']) {
    const chip = dialog.getByRole('button', { name: new RegExp(`^${size}\\b`) })
    if ((await chip.getAttribute('aria-pressed')) !== 'true') await chip.click()
  }
  await dialog.getByText('Already included', { exact: true }).click()
  await dialog.getByRole('button', { name: 'Add 6 pieces' }).click()
  await expect(dialog).toHaveCount(0)
  const names = await page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.engine.document().pieces.map((p: { name: string }) => p.name),
  )
  expect(names[0]).toBe('Front bodice (8)')
  // Nothing chosen: every line in ink.
  await page.keyboard.press('Escape')
  await page.waitForTimeout(200)
  // Every edge of the smallest front shows, through the bigger sizes'
  // paper laid over it.
  const edges: string[] = await page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.scene()[0].view.edges.map((e: { id: string }) => e.id),
  )
  expect(edges.length).toBeGreaterThan(2)
  for (const edge of edges) {
    const darkest = await darkestNear(page, await edgeMiddle(page, 0, edge))
    // Ink, not paper (hidden, it's 130 or more: a stitching line's dashes
    // nearby at most).
    expect(darkest, edge).toBeLessThan(110)
  }
})

test('sizes on the table blended, the waist cut to a measurement', async ({ page }) => {
  test.setTimeout(120_000)
  // A skirt in sizes 38 and 42, side by side on the table.
  await page.goto('/app/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('aama-graded-skirt.dxf'))
  const dialog = page.getByRole('dialog')
  for (const size of ['36', '38', '40', '42']) {
    const chip = dialog.getByRole('button', { name: new RegExp(`^${size}\\b`) })
    const pressed = (await chip.getAttribute('aria-pressed')) === 'true'
    if (pressed !== (size === '38' || size === '42')) await chip.click()
  }
  await dialog
    .getByRole('radiogroup', { name: /seam allowance included/ })
    .getByText('Not included', { exact: true })
    .click()
  await dialog.getByRole('button', { name: 'Add 4 pieces' }).click()
  await expect(dialog).toHaveCount(0)

  const list = page.getByRole('treegrid', { name: 'Pieces' })
  await list.getByRole('row', { name: /^Skirt front \(38\)/ }).click()
  await expect(page.getByText('This pattern is on the table in sizes 38 and 42.')).toBeVisible()
  await page.getByRole('button', { name: 'Blend between sizes…' }).click()
  const finder = dialog.getByRole('region', { name: 'Which size fits?' })
  await finder.getByRole('button', { name: /^Sew and measure every size/ }).click()
  await expect(finder.getByRole('table').getByRole('row')).toHaveCount(3, { timeout: 90_000 })
  // On the table, nothing to choose but the blend.
  await expect(finder.getByRole('button', { name: 'Choose 42' })).toHaveCount(0)

  const cut = finder.getByRole('group', { name: 'Size to cut at each' })
  await cut.getByRole('button', { name: /Hip/ }).click()
  await page.getByRole('option', { name: '42', exact: true }).click()
  await cut.getByRole('button', { name: /Waist/ }).click()
  await page.getByRole('option', { name: 'A measurement…' }).click()
  const waist = cut.getByRole('textbox', { name: 'Body round the waist' })
  await waist.fill('75')
  await waist.press('Tab')
  await expect(
    cut.getByText(/^Cut to fit it as fitted should: 79\.0 cm round, between 38 and 42\.$/),
  ).toBeVisible()
  await cut.getByRole('button', { name: 'Blend between them' }).click()
  await expect(dialog).toHaveCount(0, { timeout: 60_000 })
  await expect(
    page.getByText(
      '2 pieces blended between sizes: a 75.0 cm waist (between 38 and 42) and 42 at the hip.',
    ),
  ).toBeVisible()
  // Added beside the sizes, which stay.
  await expect(list.getByRole('row', { level: 1 })).toHaveText([
    /^Skirt front \(38\)/,
    /^Skirt back \(38\)/,
    /^Skirt front \(42\)/,
    /^Skirt back \(42\)/,
    /^Skirt front/,
    /^Skirt back/,
  ])
  // The front's waist between size 38's (180 mm) and 42's (200 mm); its
  // hem size 42's (285 mm).
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
    }
  })
  expect(widths.waist, `waist ${widths.waist}`).toBeGreaterThan(182)
  expect(widths.waist, `waist ${widths.waist}`).toBeLessThan(198)
  expect(Math.abs(widths.hem - 285), `hem ${widths.hem}`).toBeLessThan(1)
})
