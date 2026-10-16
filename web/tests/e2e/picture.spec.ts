import { readFile } from 'node:fs/promises'
import { expect, type Page, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

/** Opens the avatar; whether this browser draws it in 3D (not WebKit on Linux, as in CI). */
async function openTheAvatar(page: Page): Promise<boolean> {
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drawnWith = pane.getByText(/^Drawn with (WebGPU|WebGL2)$/)
  const cannot = pane.getByText('This browser can’t draw the avatar in 3D.')
  await expect(drawnWith.or(cannot)).toBeVisible({ timeout: 60_000 })
  return drawnWith.isVisible()
}

/** How many distinct colours a picture has: a blank one has one or two. */
async function colours(page: Page, bytes: Buffer, type: string): Promise<number> {
  return page.evaluate(
    async ([data, type]) => {
      const image = new Image()
      image.src = `data:${type};base64,${data}`
      await image.decode()
      const canvas = new OffscreenCanvas(image.width, image.height)
      const context = canvas.getContext('2d')!
      context.drawImage(image, 0, 0)
      const p = context.getImageData(0, 0, image.width, image.height).data
      const seen = new Set<number>()
      for (let i = 0; i < p.length; i += 16) seen.add((p[i]! << 16) | (p[i + 1]! << 8) | p[i + 2]!)
      return seen.size
    },
    [bytes.toString('base64'), type] as const,
  )
}

test('a picture and a turning video of the garment on the figure are saved', async ({ page }) => {
  test.setTimeout(240_000)
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  await expect(
    page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', { level: 1 }),
  ).toHaveCount(2)
  test.skip(!(await openTheAvatar(page)), 'Pictures are of the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  await expect(pane.getByRole('status').first()).toHaveText(/standard figure/, { timeout: 60_000 })
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(pane.getByRole('status').last()).toHaveText('Draped in poplin.', {
    timeout: 120_000,
  })

  const picture = page.waitForEvent('download')
  await pane.getByRole('button', { name: 'Save a picture of the figure' }).click()
  const png = await picture
  expect(png.suggestedFilename()).toBe('Test skirt on the figure.png')
  const bytes = await readFile((await png.path())!)
  expect(bytes.subarray(1, 4).toString()).toBe('PNG')
  // The figure and the skirt, not a blank view.
  expect(await colours(page, bytes, 'image/png')).toBeGreaterThan(50)
  await expect(page.getByText('Saved “Test skirt on the figure.png”.')).toBeVisible()

  const video = page.waitForEvent('download', { timeout: 60_000 })
  await pane.getByRole('button', { name: 'Save a video turning round the figure' }).click()
  const clip = await video
  expect(clip.suggestedFilename()).toMatch(/^Test skirt on the figure\.(webm|mp4)$/)
  const size = (await readFile((await clip.path())!)).length
  expect(size).toBeGreaterThan(20_000)
})
