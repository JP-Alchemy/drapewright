import { expect, type Page, test } from './fixtures.ts'
import { openBackup } from './helpers.ts'

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
  await page.goto('/app/')
  await openBackup(page, 'skirt.drapewright', 2)
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

  // One piece at a time, slid to the middle; the rest dimmed.
  const pieces = projector.getByRole('group', { name: 'Piece by piece' })
  await pieces.getByRole('button', { name: 'Next piece' }).click()
  await expect(pieces).toContainText(/^.*Skirt (front|back) \(1 of 2\)/)
  await pieces.getByRole('button', { name: 'Next piece' }).click()
  await expect(pieces).toContainText('(2 of 2)')
  await pieces.getByRole('button', { name: 'All' }).click()
  await expect(pieces).toContainText('All pieces')
  // Thick lines light more of the fabric.
  const thin = await lit(page)
  await projector.getByText('Thick lines (for dark fabric)').click()
  await expect.poll(() => lit(page)).toBeGreaterThan(thin)

  // The cutting layout: the fabric's fold and selvages to lay it along.
  await projector.getByText('Cutting layout', { exact: true }).click()
  await expect(projector.getByText(/^Fabric 150 cm wide, folded: lay its fold/)).toBeVisible()
  await expect.poll(() => lit(page)).toBeGreaterThan(500)
  await pieces.getByRole('button', { name: 'Next piece' }).click()
  await expect(pieces).toContainText('(1 of 2)')
  await projector.getByRole('button', { name: 'Close' }).click()
  await expect(projector).toHaveCount(0)

  // Calibrated: projected straight away next time.
  await page.getByRole('button', { name: 'Projector', exact: true }).click()
  await expect(page.getByRole('img', { name: 'The pieces, true size.' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog', { name: 'Projector' })).toHaveCount(0)
})

test.describe('on one screen', () => {
  // The screen stays its size when the page changes size, as a real one
  // does.
  test.use({ screen: { width: 1920, height: 1080 } })

  test('leaving full screen while measuring leaves the projector open', async ({ page }) => {
    await page.goto('/app/')
    await openBackup(page, 'skirt.drapewright', 2)
    await page.getByRole('button', { name: 'Projector', exact: true }).click()
    const projector = page.getByRole('dialog', { name: 'Projector' })
    await expect(projector.getByText('Calibrate the projector')).toBeVisible()
    // Into full screen, by its button where the browser didn't start there.
    const fullScreen = projector.getByRole('button', { name: 'Full screen' })
    const inFullScreen = () => page.evaluate(() => document.fullscreenElement !== null)
    // (The projector may go full screen itself meanwhile, the button gone.)
    if (!(await inFullScreen())) await fullScreen.click({ timeout: 2000 }).catch(() => {})
    await page.waitForTimeout(300)
    const across = projector.getByRole('textbox', { name: 'Rectangle across' })
    if (!(await inFullScreen())) {
      // A browser that won't go full screen here (WebKit under test):
      // typing keeps the projector open, full screen still offered.
      await across.fill('50')
      await across.press('Tab')
      await expect(projector.getByText('Calibrate the projector')).toBeVisible()
      await expect(across).toHaveValue('50')
      await expect(fullScreen).toBeVisible()
      return
    }
    await expect(fullScreen).toBeHidden()
    // A corner dragged onto the mat's mark.
    const corners = () =>
      page.evaluate(
        () => JSON.parse(localStorage.getItem('drapewright.projector') ?? 'null')?.corners,
      )
    const box = (await page.getByRole('img', { name: /calibrate by/ }).boundingBox())!
    const s = Math.min((box.width * 0.7) / 400, (box.height * 0.7) / 300)
    const corner = { x: box.x + (box.width - 400 * s) / 2, y: box.y + (box.height - 300 * s) / 2 }
    await page.mouse.move(corner.x, corner.y)
    await page.mouse.down()
    await page.mouse.move(corner.x + 30, corner.y + 20, { steps: 4 })
    await page.mouse.up()
    const dragged = await corners()
    expect(dragged).not.toBeNull()
    // A measurement typed, and full screen left as Safari leaves it then.
    await across.fill('50')
    await page.evaluate(() => document.exitFullscreen())
    await expect.poll(inFullScreen).toBe(false)
    // Out of full screen, the page is smaller (the browser's bar is back),
    // on the same screen.
    const viewport = page.viewportSize()!
    const cdp = await page.context().newCDPSession(page)
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: viewport.width,
      height: viewport.height - 80,
      deviceScaleFactor: 1,
      mobile: false,
      screenWidth: 1920,
      screenHeight: 1080,
    })
    await across.press('Tab')
    // Still open, still calibrating, the corner where it was dragged and the
    // measurement kept; back into full screen at a tap, the same.
    await expect(projector.getByText('Calibrate the projector')).toBeVisible()
    await expect(across).toHaveValue('50')
    expect(await corners()).toEqual(dragged)
    await fullScreen.click()
    await expect.poll(inFullScreen).toBe(true)
    expect(await corners()).toEqual(dragged)
    // Escape while typing doesn't close it; after, it does.
    await across.focus()
    await across.press('Escape')
    await expect(projector).toBeVisible()
    // Done: the calibration kept is the one made, the corner as dragged.
    await projector.getByRole('button', { name: 'Done' }).click()
    expect(await corners()).toEqual(dragged)
    await projector.getByRole('button', { name: 'Close' }).click()
    await expect(projector).toHaveCount(0)
  })
})

test('the calibration is kept, and corners and the pattern step a little at a time', async ({
  page,
}) => {
  await page.goto('/app/')
  await openBackup(page, 'skirt.drapewright', 2)
  await page.getByRole('button', { name: 'Projector', exact: true }).click()
  const projector = page.getByRole('dialog', { name: 'Projector' })
  await expect(projector.getByText('Calibrate the projector')).toBeVisible()
  const saved = () =>
    page.evaluate(() =>
      Object.values(
        JSON.parse(localStorage.getItem('drapewright.projector.calibrations') ?? '{}'),
      ).map((c) => (c as { corners: [number, number][] }).corners),
    )
  // The top left corner, a centimetre right and one down: 40 cm across
  // the rectangle's width on the page, each step its 400th.
  await projector.getByText('1 cm', { exact: true }).click()
  const corner = projector.getByRole('group', { name: 'Move the top left corner' })
  await corner.getByRole('button', { name: 'Right' }).click()
  await corner.getByRole('button', { name: 'Down' }).click()
  const [once] = await saved()
  expect(once).toBeDefined()
  // The bottom right corner, by the arrow keys: Shift for ten steps.
  await projector.getByText('Bottom right', { exact: true }).click()
  await expect(projector.getByRole('group', { name: 'Move the bottom right corner' })).toBeVisible()
  // (Off the controls: there, the arrow keys move between their choices.)
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())
  await page.keyboard.press('Shift+ArrowLeft')
  const [twice] = await saved()
  const [a, b] = [once!, twice!]
  // A step is 10 mm at the rectangle's scale, the same either way.
  const step = Math.hypot(a[1]![0] - a[0]![0], a[1]![1] - a[0]![1]) / 40
  // (Near enough: the scale is taken along its top and side, both.)
  expect(Math.abs(a[0]![0] - a[3]![0] - step)).toBeLessThan(2)
  expect(Math.abs(b[2]![0] - (a[2]![0] - 10 * step))).toBeLessThan(2)
  await projector.getByRole('button', { name: 'Done' }).click()
  await expect(page.getByText('Calibration saved for this screen.')).toBeVisible()

  // Closed and opened again: ready to cut, calibrated as it was.
  await projector.getByRole('button', { name: 'Close' }).click()
  await page.getByRole('button', { name: 'Projector', exact: true }).click()
  await expect(projector.getByText('Calibrate the projector')).toHaveCount(0)
  await expect(page.getByRole('img', { name: 'The pieces, true size.' })).toBeVisible()
  await expect(projector.getByText(/The calibration is saved on this device/)).toBeVisible()
  expect(await saved()).toEqual([b])
  // The pattern stepped across the fabric.
  const picture = () => page.getByRole('img', { name: 'The pieces, true size.' }).screenshot()
  const before = await picture()
  await projector
    .getByRole('group', { name: 'Move the pattern' })
    .getByRole('button', { name: 'Right' })
    .click()
  await expect.poll(async () => (await picture()).equals(before)).toBe(false)
})
