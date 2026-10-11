import { expect, type Page, test } from '@playwright/test'
import { drawnInTheMiddle, fixture, openWithFixture } from './helpers.ts'

/** The share of two screenshots' pixels that differ, 0 to 1. */
async function differing(page: Page, a: Buffer, b: Buffer): Promise<number> {
  return page.evaluate(
    async ([a, b]) => {
      const pixels = async (data: string) => {
        const image = new Image()
        image.src = `data:image/png;base64,${data}`
        await image.decode()
        const canvas = new OffscreenCanvas(image.width, image.height)
        const context = canvas.getContext('2d')!
        context.drawImage(image, 0, 0)
        return context.getImageData(0, 0, image.width, image.height).data
      }
      const [p, q] = [await pixels(a), await pixels(b)]
      if (p.length !== q.length) return 1
      let changed = 0
      for (let i = 0; i < p.length; i += 4) {
        const d =
          Math.abs(p[i]! - q[i]!) +
          Math.abs(p[i + 1]! - q[i + 1]!) +
          Math.abs(p[i + 2]! - q[i + 2]!)
        if (d > 30) changed++
      }
      return changed / (p.length / 4)
    },
    [a.toString('base64'), b.toString('base64')],
  )
}

/** Opens the avatar; whether this browser draws it in 3D (not WebKit on Linux, as in CI). */
async function openTheAvatar(page: Page): Promise<boolean> {
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drawnWith = pane.getByText(/^Drawn with (WebGPU|WebGL2)$/)
  const cannot = pane.getByText('This browser can’t draw the avatar in 3D.')
  await expect(drawnWith.or(cannot)).toBeVisible({ timeout: 60_000 })
  return drawnWith.isVisible()
}

test('the skirt is draped, read where it pulls, draped in another fabric, and taken off', async ({
  page,
}) => {
  test.setTimeout(240_000)
  await page.goto('/')
  // A backup of the test skirt: a front and a back, their seams sewn.
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(2)
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')

  const pane = page.getByRole('region', { name: 'Avatar' })
  const view = pane.getByRole('img', { name: /standard figure/ })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const status = drape.getByRole('status')
  await expect(pane.getByRole('status').first()).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )
  // The figure as drawn, bare: on the worker's next frame after the status.
  let bare = await view.screenshot()
  await expect
    .poll(async () => drawnInTheMiddle(page, (bare = await view.screenshot())), {
      timeout: 10_000,
    })
    .toBe(true)
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(status).toHaveText(/Sewing the seams…|Letting it settle…|Draped in poplin\./)
  await expect(status).toHaveText('Draped in poplin.', { timeout: 120_000 })
  // The skirt is drawn on the figure.
  await expect
    .poll(async () => differing(page, bare, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.02)

  // Where it pulls, and the room it has: heatmaps, read where tapped.
  const inFabric = await view.screenshot()
  await drape.getByText('Pulling', { exact: true }).click()
  await expect(drape.getByRole('img', { name: 'Pulling: from None to Hard' })).toBeVisible()
  await expect
    .poll(async () => differing(page, inFabric, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.02)
  // The middle of the view is the middle of the figure: the skirt's front.
  const box = (await view.boundingBox())!
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2)
  await expect(status).toHaveText(
    /^Here it is (hardly pulled|pulled lightly|pulled firmly|pulled hard) \(\d+ N\/m\), stretched \d+\.\d%, and (resting on the body|[\d.]+ cm from the body)\.$/,
  )
  await drape.getByText('Room', { exact: true }).click()
  await expect(drape.getByRole('img', { name: 'Room: from Touching to 4.0 cm away' })).toBeVisible()

  // Another fabric: draped again in it.
  await drape.getByRole('button', { name: /Fabric/ }).click()
  await page.getByRole('option', { name: 'Denim' }).click()
  await expect(status).toHaveText('Draped in denim.', { timeout: 120_000 })

  await drape.getByRole('button', { name: 'Take off' }).click()
  await expect(status).toHaveText('')
  await expect(drape.getByRole('button', { name: 'Drape', exact: true })).toBeEnabled()
  await expect
    .poll(async () => differing(page, bare, await view.screenshot()), { timeout: 10_000 })
    .toBeLessThan(0.002)
})

test('nothing is draped until something is sewn', async ({ page }) => {
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')
  const drape = page
    .getByRole('region', { name: 'Avatar' })
    .getByRole('group', { name: 'Drape the garment' })
  const button = drape.getByRole('button', { name: 'Drape', exact: true })
  await expect(button).toBeEnabled({ timeout: 60_000 })
  await button.click()
  await expect(drape.getByRole('status')).toHaveText(
    'Nothing is sewn yet: sew the pieces’ edges together first.',
  )
})
