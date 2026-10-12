import { readFile } from 'node:fs/promises'
import { expect, test } from './fixtures.ts'
import { edgeMiddle, edgesOf, fixture, notchesOf, openWithFixture } from './helpers.ts'

/** The project as saved: what she would get back after closing the app. */
const saved = (page: import('@playwright/test').Page) =>
  page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.engine.document(),
  )

test('Phase 1 walk-through: open, adjust, mark, print, and find it again', async ({ page }) => {
  // Open a pattern drawn on its stitching line.
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  await page.getByRole('option', { name: /Flared skirt front/ }).click()

  // A little more seam allowance, a 4 cm hem.
  await page.getByRole('button', { name: 'Increase Seam allowance' }).click()
  await expect(page.getByRole('textbox', { name: 'Seam allowance' })).toHaveValue('1.6 cm')
  const hem = await edgeMiddle(page, 0, 'e4')
  await page.mouse.click(hem.x, hem.y)
  await page
    .getByRole('radiogroup', { name: 'This edge is a' })
    .getByText('Hem', { exact: true })
    .click()

  // A notch on the side seam and a grainline.
  await page.locator('canvas').focus()
  await page.keyboard.press('n')
  const side = await edgeMiddle(page, 0, 'e3')
  await page.mouse.click(side.x, side.y)
  expect((await notchesOf(page, 0)).notches).toHaveLength(1)
  await page.keyboard.press('v')
  await page.getByRole('option', { name: /Flared skirt front/ }).click()
  await page.getByRole('button', { name: 'Add a straight grainline' }).click()

  // Print it.
  await page.getByRole('button', { name: 'Print', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Print at true size' })
  await expect(dialog.getByTestId('print-summary')).toContainText('of A4')
  const download = page.waitForEvent('download')
  await dialog.getByRole('button', { name: 'Save PDF' }).click()
  const pdf = (await readFile(await (await download).path())).toString('latin1')
  expect(pdf.startsWith('%PDF-')).toBe(true)
  expect(pdf).toContain('/PrintScaling /None')

  // Close and come back: everything is still there.
  await expect(page.getByTestId('save-status')).toHaveText('Saved')
  await page.reload()
  await page.waitForFunction(() => 'drapewright' in window)
  const doc = await saved(page)
  expect(doc.pieces).toHaveLength(1)
  expect(doc.pieces[0].allowance.seam).toBeCloseTo(16, 9)
  expect((await edgesOf(page, 0))[2]).toEqual(['e4', 'hem', 'auto', null])
  expect(doc.pieces[0].notches).toHaveLength(1)
  expect(doc.pieces[0].grainline).not.toBeNull()
  expect(doc.print.paper).toBe('a4')
})

test('works offline after the first visit', async ({ page, context }, info) => {
  test.skip(info.project.name !== 'chromium', 'Playwright drives service workers in Chromium')
  await page.goto('/?debug')
  // The service worker has stored the app and taken charge of the page.
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null)
  await context.setOffline(true)
  await page.reload()
  await page.waitForFunction(() => 'drapewright' in window)
  await page.getByTestId('pattern-file').setInputFiles(fixture('inkscape-mm.svg'))
  const dialog = page.getByRole('dialog')
  await dialog.getByText('Not included', { exact: true }).click()
  await dialog.getByRole('button', { name: 'Add 2 pieces' }).click()
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(2)
  await context.setOffline(false)
})

test('an edit and its redrawn lines take less than a frame', async ({ page }) => {
  // Straight and curved pieces, both allowance modes.
  await openWithFixture(page, 'aama-skirt.dxf', 'Add 2 pieces', 'Already included')
  for (const [file, add] of [
    ['flared-skirt.svg', 'Add the piece'],
    ['inkscape-mm.svg', 'Add 2 pieces'],
  ] as const) {
    await page.getByTestId('pattern-file').setInputFiles(fixture(file))
    const dialog = page.getByRole('dialog')
    await dialog.getByText('Not included', { exact: true }).click()
    await dialog.getByRole('button', { name: add }).click()
    await expect(dialog).toBeHidden()
  }

  const timing = await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const times: number[] = []
    for (let i = 0; i < 30; i++) {
      for (const piece of s.engine.document().pieces) {
        const before = s.scenePiece(piece.id).view
        const start = performance.now()
        s.dispatch({
          type: 'setAllowance',
          piece: piece.id,
          allowance: { ...piece.allowance, seam: 10 + (i % 6) },
        })
        // The engine's derived lines, and the paths the canvas draws.
        s.scene()
        times.push(performance.now() - start)
        if (s.scenePiece(piece.id).view === before) throw new Error('the piece was not redrawn')
      }
    }
    times.sort((a, b) => a - b)
    return { median: times[times.length >> 1]!, worst: times[times.length - 1]! }
  })
  test.info().annotations.push({
    type: 'edit time',
    description: `median ${timing.median.toFixed(2)} ms, worst ${timing.worst.toFixed(2)} ms`,
  })
  expect(timing.median).toBeLessThan(16)
})
