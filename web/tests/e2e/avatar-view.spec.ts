import { expect, type Locator, type Page, test } from '@playwright/test'
import { drawnInTheMiddle } from './helpers.ts'

/** The figure is drawn in the middle of the view, and a drag turns it. */
async function seeItDrawnAndTurned(page: Page, view: Locator) {
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
}

test('the avatar is seen in 3D, turned, and fitted to the measurements', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drawnWith = pane.getByText(/^Drawn with (WebGPU|WebGL2)$/)
  // Where the browser can't draw in 3D (WebKit on Linux, as in CI), it says
  // so, and the avatar is still fitted and reported.
  const cannot = pane.getByText('This browser can’t draw the avatar in 3D.')
  await expect(drawnWith.or(cannot)).toBeVisible({ timeout: 60_000 })
  const drawing = await drawnWith.isVisible()
  // The first status is the avatar's; the drape has its own.
  console.log(`${test.info().project.name}: ${drawing ? await drawnWith.textContent() : 'no 3D'}`)
  await expect(pane.getByRole('status').first()).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )
  const view = pane.getByRole('img', { name: /standard figure/ })
  if (drawing) await seeItDrawnAndTurned(page, view)

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
  await expect(pane.getByRole('status').first()).toHaveText('All 4 measurements within 1 cm', {
    timeout: 60_000,
  })
  if (drawing) await expect(pane.getByRole('img', { name: /in Me’s figure/ })).toBeVisible()

  await pane.getByRole('button', { name: 'Close the avatar' }).click()
  await expect(pane).toHaveCount(0)
})
