import { expect, type Page, test } from './fixtures.ts'
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
  await page.goto('/app/')
  // A backup of the test skirt: a front and a back, their seams sewn.
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(2)
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')

  const pane = page.getByRole('region', { name: 'Avatar' })
  const view = pane.getByRole('img', { name: /standard figure/ })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  // The drape's status, over the view (after the figure's, in its head).
  const status = pane.getByRole('status').last()
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
  // How big it is round the figure's waist and hip, and the ease there (a
  // skirt has no bust).
  await expect(
    pane.getByText(/^Garment: waist \d+\.\d cm \(.+\) · hip \d+\.\d cm \(.+\)$/),
  ).toBeVisible()
  // The skirt is drawn on the figure.
  await expect
    .poll(async () => differing(page, bare, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.02)

  // Where it pulls, and the room it has: heatmaps, read where tapped.
  const inFabric = await view.screenshot()
  await drape.getByText('Pulling', { exact: true }).click()
  await expect(pane.getByRole('img', { name: 'Pulling: from None to Hard' })).toBeVisible()
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
  await expect(pane.getByRole('img', { name: 'Room: from Touching to 4.0 cm away' })).toBeVisible()

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
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const button = drape.getByRole('button', { name: 'Drape', exact: true })
  await expect(button).toBeEnabled({ timeout: 60_000 })
  await button.click()
  await expect(pane.getByRole('status').last()).toHaveText(
    'Nothing is sewn yet: sew the pieces’ edges together first.',
  )
})

test('a draped garment follows changes to its pattern', async ({ page }) => {
  test.setTimeout(240_000)
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const view = pane.getByRole('img', { name: /standard figure/ })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const status = pane.getByRole('status').last()
  await expect(pane.getByRole('status').first()).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(status).toHaveText('Draped in poplin.', { timeout: 120_000 })
  const before = await view.screenshot()

  // The front lengthened 8 cm: draped again by itself, the skirt shown
  // till then.
  await page
    .getByRole('listbox', { name: 'Pieces' })
    .getByRole('option', { name: /Skirt front/ })
    .click()
  await page.getByRole('button', { name: 'Add a step' }).click()
  await page.getByRole('menuitem', { name: /^Lengthen or shorten/ }).click()
  const by = page.getByRole('textbox', { name: 'By how much' })
  await by.fill('8')
  await by.press('Enter')
  await expect(status).toHaveText('Draping it again to follow the change…')
  await expect(drape.getByRole('button', { name: 'Take off' })).toBeEnabled()
  await expect(status).toHaveText('Draped in poplin.', { timeout: 120_000 })
  await expect
    .poll(async () => differing(page, before, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.005)
})

test('a fabric of one’s own is measured, draped in, and changed', async ({ page }) => {
  test.setTimeout(240_000)
  await page.goto('/app/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const status = pane.getByRole('status').last()
  await expect(pane.getByRole('status').first()).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )

  // Measured: like linen, a 10 cm square 1.9 g, a strip sliding out 6 cm.
  await drape.getByRole('button', { name: /Fabric/ }).click()
  await page.getByRole('option', { name: 'Measure a fabric…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Measure a fabric' })
  await dialog.getByRole('textbox', { name: 'Name' }).fill('Blue linen')
  await dialog.getByRole('button', { name: /Most like/ }).click()
  await page.getByRole('option', { name: 'Linen' }).click()
  await dialog.getByRole('textbox', { name: 'A 10 cm square weighs' }).fill('1.9')
  await dialog.getByRole('textbox', { name: 'Slides out over a table’s edge' }).fill('6')
  await dialog.getByRole('button', { name: 'Save' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(drape.getByRole('button', { name: /Fabric/ })).toContainText('Blue linen')

  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(status).toHaveText('Draped in Blue linen.', { timeout: 120_000 })
  // Changed: draped again in it.
  await drape.getByRole('button', { name: /Fabric/ }).click()
  await page.getByRole('option', { name: 'Change “Blue linen”…' }).click()
  const change = page.getByRole('dialog', { name: 'Change “Blue linen”' })
  await change.getByRole('textbox', { name: 'A 10 cm square weighs' }).fill('2.4')
  await change.getByRole('button', { name: 'Save' }).click()
  await expect(status).toHaveText('Draping it again to follow the change…')
  await expect(status).toHaveText('Draped in Blue linen.', { timeout: 120_000 })
  // Kept with the project.
  const fabrics = await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    return s.engine.document().fabrics
  })
  expect(fabrics).toEqual([
    expect.objectContaining({
      name: 'Blue linen',
      like: 'linen',
      measured: { weight: 240, overhang: 6 },
    }),
  ])
})

test('the figure takes a pose, and a skirt draped on it moves with it', async ({ page }) => {
  test.setTimeout(240_000)
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  test.skip(!(await openTheAvatar(page)), 'Poses are seen in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const view = pane.getByRole('img', { name: /standard figure/ })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const status = pane.getByRole('status').last()
  await expect(pane.getByRole('status').first()).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )
  let standing = await view.screenshot()
  await expect
    .poll(async () => drawnInTheMiddle(page, (standing = await view.screenshot())), {
      timeout: 10_000,
    })
    .toBe(true)

  // Sitting, bare.
  const pose = pane.getByRole('button', { name: /Pose/ })
  await expect(pose).toContainText('Standing')
  await pose.click()
  await page.getByRole('option', { name: 'Sitting' }).click()
  await expect
    .poll(async () => differing(page, standing, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.01)

  // Draped standing, then moved into the pose.
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(status).toHaveText('Moving into the pose…', { timeout: 120_000 })
  await expect(status).toHaveText('Draped in poplin.', { timeout: 120_000 })
  const sitting = await view.screenshot()

  // Walking: the figure stands up and steps out, the skirt with it.
  await pose.click()
  await page.getByRole('option', { name: 'Walking' }).click()
  await expect(status).toHaveText(/Moving into the pose…|Letting it settle…/)
  await expect(status).toHaveText('Draped in poplin.', { timeout: 120_000 })
  await expect
    .poll(async () => differing(page, sitting, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.01)
})

test('a shirt drapes with its facing inside, buttoned and then open', async ({ page }) => {
  test.setTimeout(240_000)
  await page.goto('/app/')
  // The test shirt: buttoned down the front, a facing at the back of its neck.
  await page.getByTestId('pattern-file').setInputFiles(fixture('shirt.drapewright'))
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const view = pane.getByRole('img', { name: /standard figure/ })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const status = pane.getByRole('status').last()
  await expect(pane.getByRole('status').first()).toHaveText(
    'A standard figure: add measurements to see someone’s.',
    { timeout: 60_000 },
  )
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(status).toHaveText('Draped in poplin. Inside, shown paler: Back neck facing.', {
    timeout: 120_000,
  })
  // With WebGPU, the cloth is moved on on the graphics card too.
  if (await pane.getByText(/with WebGPU$/).isVisible()) {
    await expect(pane.getByText('Drawn and draped with WebGPU')).toBeVisible()
  }
  const buttoned = await view.screenshot()
  const doneUp = drape.getByRole('switch', { name: 'Buttons done up' })
  await expect(doneUp).toBeChecked()
  await drape.getByText('Buttons done up').click()
  await expect(doneUp).not.toBeChecked()
  await expect(status).toHaveText(/Sewing the seams…|Letting it settle…/)
  await expect(status).toHaveText('Draped in poplin. Inside, shown paler: Back neck facing.', {
    timeout: 120_000,
  })
  await expect
    .poll(async () => differing(page, buttoned, await view.screenshot()), { timeout: 10_000 })
    .toBeGreaterThan(0.002)
})

test('a lined skirt drapes with its lining inside', async ({ page }) => {
  test.setTimeout(240_000)
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('lined-skirt.drapewright'))
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const status = pane.getByRole('status').last()
  await expect(pane.getByRole('status').first()).toHaveText(/standard figure/, {
    timeout: 60_000,
  })
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(status).toHaveText(
    'Draped in poplin. Inside, shown paler: Skirt front lining and Skirt back lining.',
    { timeout: 120_000 },
  )
  // No buttons to undo.
  await expect(drape.getByRole('switch', { name: 'Buttons done up' })).toHaveCount(0)
})
