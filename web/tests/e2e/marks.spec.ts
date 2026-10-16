import { expect, test } from './fixtures.ts'
import { edgeMiddle, notchesOf, openWithFixture, toPage } from './helpers.ts'

test('the notch tool adds, cycles and slides notches', async ({ page }, info) => {
  test.skip(info.project.name === 'ipad-webkit', 'mouse drag')
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  await page.locator('canvas').focus()
  await page.keyboard.press('n')
  const side = await edgeMiddle(page, 0, 'e3')
  await page.mouse.click(side.x, side.y)
  let state = await notchesOf(page, 0)
  expect(state.notches).toHaveLength(1)
  expect(state.notches[0]?.[0]).toBe('single')
  await expect(page.getByRole('heading', { name: 'Notch', exact: true })).toBeVisible()

  // Tap the visible slit, out at the cutting line: double.
  const cut = await toPage(page, state.firstCut![0], state.firstCut![1])
  await page.mouse.click(cut.x, cut.y)
  state = await notchesOf(page, 0)
  expect(state.notches[0]?.[0]).toBe('double')

  // Drag it along the edge: one undo step.
  const before = state.notches[0]![1] as number
  await page.mouse.move(cut.x, cut.y)
  await page.mouse.down()
  await page.mouse.move(cut.x + 20, cut.y + 60, { steps: 6 })
  await page.mouse.up()
  const after = (await notchesOf(page, 0)).notches[0]![1] as number
  expect(Math.abs(after - before)).toBeGreaterThan(20)
  await page.getByRole('button', { name: 'Undo change notch' }).click()
  expect((await notchesOf(page, 0)).notches[0]![1]).toBeCloseTo(before, 6)
})

test('the grainline tool snaps to the piece’s axes', async ({ page }, info) => {
  test.skip(info.project.name === 'ipad-webkit', 'mouse drag')
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  await page.locator('canvas').focus()
  await page.keyboard.press('g')
  const a = await toPage(page, 120, 150)
  const b = await toPage(page, 126, 450)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  await page.mouse.move(b.x, b.y, { steps: 8 })
  await page.mouse.up()
  const g = await page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => (window as any).drapewright.engine.document().pieces[0].grainline,
  )
  expect(g).not.toBeNull()
  // A few degrees off vertical snaps to vertical.
  expect(Math.abs(g.to[0] - g.from[0])).toBeLessThan(1e-6)
  await expect(page.getByRole('switch', { name: /One way/ })).toBeVisible()
})

test('cutting instructions', async ({ page }) => {
  await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
  await page.getByRole('row', { name: /Waistband/, level: 1 }).click()
  await expect(page.getByRole('switch', { name: 'As mirrored pairs' })).toHaveCount(0)
  await page.getByRole('button', { name: 'Increase How many' }).click()
  await expect(page.getByRole('switch', { name: 'As mirrored pairs' })).toBeChecked()
  await page.getByRole('button', { name: 'Also cut in another fabric' }).click()
  await expect(page.getByRole('button', { name: 'Increase How many' })).toHaveCount(2)
  const cut = await page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => (window as any).drapewright.engine.document().pieces[1].cut,
  )
  expect(cut.map((c: { material: string; quantity: number }) => [c.material, c.quantity])).toEqual([
    ['main', 2],
    ['lining', 2],
  ])
})
