import { fileURLToPath } from 'node:url'
import { expect, type Locator, type Page, test } from './fixtures.ts'

/** A paper pattern piece photographed at a slant beside a sheet of A4. */
const PHOTO = fileURLToPath(new URL('../../../fixtures/photo/piece-and-a4.png', import.meta.url))
/** The photo's size, the sheet's corners in it, and a point on the piece (px). */
const SIZE = [720, 900] as const
const CORNERS = [
  [205, 140],
  [70, 150],
  [55, 352],
  [222, 335],
] as const
const ON_THE_PIECE = [352, 324] as const

/** Taps the photo at `p` (its own px), where the picture shows it. */
async function tap(page: Page, picture: Locator, [x, y]: readonly [number, number]) {
  const box = (await picture.boundingBox())!
  await page.mouse.click(box.x + (x / SIZE[0]) * box.width, box.y + (y / SIZE[1]) * box.height)
}

test('a paper pattern piece is found in a photo and added at its size', async ({ page }) => {
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(PHOTO)
  const dialog = page.getByRole('dialog')
  await expect(dialog.getByRole('heading', { name: 'A pattern piece from a photo' })).toBeVisible()
  const status = dialog.getByRole('status').first()
  await expect(status).toHaveText('Tap the four corners of the sheet of paper.')
  const photo = dialog.getByRole('img', { name: 'The photo piece-and-a4.png' })
  for (const [k, corner] of CORNERS.entries()) {
    await tap(page, photo, corner)
    if (k === 1) await expect(status).toHaveText('Tap 2 more corners of the sheet.')
  }
  await expect(status).toHaveText('Now tap the pattern piece. Drag a corner to move it.')
  const use = dialog.getByRole('button', { name: 'Use this piece' })
  await expect(use).toBeDisabled()
  await tap(page, photo, ON_THE_PIECE)
  await expect(status).toHaveText(/outline is shown in orange/)
  await dialog.getByRole('textbox', { name: 'Name' }).fill('Skirt front')
  await use.click()

  // Then imported as any pattern is: is allowance included?
  await expect(dialog.getByRole('heading', { name: 'Found 1 piece' })).toBeVisible()
  await expect(dialog).toContainText('Measured against the sheet of paper in the photo.')
  await dialog.getByText('Already included', { exact: true }).click()
  await dialog.getByRole('button', { name: 'Add the piece' }).click()
  const piece = page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', {
    name: /Skirt front/,
    level: 1,
  })
  // 27 cm × 55.5 cm, as photographed (within a few millimetres).
  await expect(piece).toContainText(/2[67]\.\d cm × 55\.\d cm/)
})

test('tapping the sheet instead of the piece is explained', async ({ page }) => {
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(PHOTO)
  const dialog = page.getByRole('dialog')
  const photo = dialog.getByRole('img', { name: 'The photo piece-and-a4.png' })
  for (const corner of CORNERS) await tap(page, photo, corner)
  await tap(page, photo, [140, 250])
  await expect(dialog.getByRole('status').first()).toHaveText(
    'The piece couldn’t be found in the photo: the point tapped is on the sheet of paper: tap the pattern piece.',
  )
  await dialog.getByRole('button', { name: 'Start again' }).click()
  await expect(dialog.getByRole('status').first()).toHaveText(
    'Tap the four corners of the sheet of paper.',
  )
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(dialog).toHaveCount(0)
})
