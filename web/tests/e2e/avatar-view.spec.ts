import { expect, type Page, test } from '@playwright/test'

/** Whether a screenshot shows something in its middle unlike its corner. */
async function drawnInTheMiddle(page: Page, png: Buffer): Promise<boolean> {
  return page.evaluate(async (data) => {
    const image = new Image()
    image.src = `data:image/png;base64,${data}`
    await image.decode()
    const canvas = new OffscreenCanvas(image.width, image.height)
    const context = canvas.getContext('2d')!
    context.drawImage(image, 0, 0)
    const at = (x: number, y: number) => context.getImageData(x, y, 1, 1).data
    const corner = at(4, 4)
    // Down the middle, where the figure stands.
    for (let y = Math.round(image.height * 0.3); y < image.height * 0.7; y += 4) {
      const p = at(Math.round(image.width / 2), y)
      const difference = [0, 1, 2].reduce((d, i) => d + Math.abs(p[i]! - corner[i]!), 0)
      if (difference > 30) return true
    }
    return false
  }, png.toString('base64'))
}

test('the avatar is seen in 3D, turned, and fitted to the measurements', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drawnWith = pane.getByText(/^Drawn with (WebGPU|WebGL2)$/)
  await expect(drawnWith).toBeVisible({ timeout: 60_000 })
  // Which way it drew here: WebGPU, or the WebGL2 fallback.
  console.log(`${test.info().project.name}: ${await drawnWith.textContent()}`)
  await expect(pane.getByRole('status')).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )
  const view = pane.getByRole('img', { name: /standard figure/ })
  // Drawn on the worker's next frame, after the status says so.
  let before = await view.screenshot()
  await expect
    .poll(async () => drawnInTheMiddle(page, (before = await view.screenshot())), {
      timeout: 10_000,
    })
    .toBe(true)

  // A drag across the figure turns it.
  const box = (await view.boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.down()
  await page.mouse.move(box.x + box.width / 2 + 150, box.y + box.height / 2, { steps: 6 })
  await page.mouse.up()
  await expect
    .poll(async () => !(await view.screenshot()).equals(before), { timeout: 10_000 })
    .toBe(true)

  // Someone's measurements: the avatar takes their figure.
  await page.getByRole('button', { name: 'Measurements' }).click()
  const dialog = page.getByRole('dialog', { name: 'Measurements' })
  await dialog.getByRole('button', { name: 'Add my measurements' }).click()
  for (const [name, cm] of [
    ['Height', '160'],
    ['Bust', '100'],
    ['Waist', '82'],
    ['Hip', '106'],
  ] as const) {
    const field = dialog.getByRole('textbox', { name, exact: true })
    await field.fill(cm)
    await field.press('Tab')
  }
  await dialog.getByRole('button', { name: 'Done' }).click()
  await expect(pane.getByRole('status')).toHaveText('All 4 measurements within 1 cm', {
    timeout: 60_000,
  })
  await expect(pane.getByRole('img', { name: /in Me’s figure/ })).toBeVisible()

  await pane.getByRole('button', { name: 'Close the avatar' }).click()
  await expect(pane).toHaveCount(0)
})
