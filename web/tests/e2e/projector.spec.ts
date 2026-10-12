import { expect, type Page, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

/** How many of the projection's pixels are lit (not black). */
async function lit(page: Page): Promise<number> {
  const shot = await page.getByRole('img', { name: /true size|calibrate by/ }).screenshot()
  return page.evaluate(async (data) => {
    const image = new Image()
    image.src = `data:image/png;base64,${data}`
    await image.decode()
    const canvas = new OffscreenCanvas(image.width, image.height)
    const context = canvas.getContext('2d')!
    context.drawImage(image, 0, 0)
    const p = context.getImageData(0, 0, image.width, image.height).data
    let n = 0
    for (let i = 0; i < p.length; i += 4) if (p[i]! + p[i + 1]! + p[i + 2]! > 200) n++
    return n
  }, shot.toString('base64'))
}

test('the pieces are projected true size once the projector is calibrated', async ({ page }) => {
  await page.goto('/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  await page.getByRole('button', { name: 'Projector', exact: true }).click()
  const projector = page.getByRole('dialog', { name: 'Projector' })
  await expect(projector.getByText('Calibrate the projector')).toBeVisible()

  // A corner dragged onto the mat's mark, and the rectangle's size given.
  const box = (await page.getByRole('img', { name: /calibrate by/ }).boundingBox())!
  const [w, h] = [box.width, box.height]
  const s = Math.min((w * 0.7) / 400, (h * 0.7) / 300)
  const corner = { x: box.x + (w - 400 * s) / 2, y: box.y + (h - 300 * s) / 2 }
  await page.mouse.move(corner.x, corner.y)
  await page.mouse.down()
  await page.mouse.move(corner.x + 20, corner.y + 12, { steps: 4 })
  await page.mouse.up()
  const across = projector.getByRole('textbox', { name: 'Rectangle across' })
  await across.fill('50')
  await across.press('Enter')
  await projector.getByRole('button', { name: 'Done' }).click()

  // The pieces, drawn in bright lines on black.
  await expect(page.getByRole('img', { name: 'The pieces, true size.' })).toBeVisible()
  await expect.poll(() => lit(page)).toBeGreaterThan(500)
  // Slid across the fabric.
  const before = await lit(page)
  await page.mouse.move(box.x + w / 2, box.y + h / 2)
  await page.mouse.down()
  await page.mouse.move(box.x + w / 2 + 300, box.y + h / 2, { steps: 5 })
  await page.mouse.up()
  await expect.poll(() => lit(page)).not.toBe(before)
  await projector.getByRole('button', { name: 'Close' }).click()
  await expect(projector).toHaveCount(0)

  // Calibrated: projected straight away next time.
  await page.getByRole('button', { name: 'Projector', exact: true }).click()
  await expect(page.getByRole('img', { name: 'The pieces, true size.' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog', { name: 'Projector' })).toHaveCount(0)
})
