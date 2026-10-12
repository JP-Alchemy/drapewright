import { expect, test } from './fixtures.ts'
import { modKey, multiTap, openWithFixture, pieceCentre, placementOf } from './helpers.ts'

test.describe('moving pieces', () => {
  test('dragging a piece moves it as one undo step', async ({ page }, info) => {
    test.skip(info.project.name === 'ipad-webkit', 'mouse drag')
    await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
    const from = await pieceCentre(page, 0)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(from.x + 40, from.y + 30, { steps: 5 })
    await page.mouse.up()
    const moved = await placementOf(page, 0)
    expect(moved.x).toBeGreaterThan(5)
    expect(moved.y).toBeGreaterThan(5)
    await page.getByRole('button', { name: 'Undo move' }).click()
    expect((await placementOf(page, 0)).x).toBe(0)
  })

  test('arrow keys nudge the selection; a run of nudges is one undo step', async ({ page }) => {
    await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
    await page.getByRole('option', { name: /Waistband/ }).click()
    await page.locator('canvas').focus()
    for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowRight')
    await page.keyboard.press('Shift+ArrowDown')
    const nudged = await placementOf(page, 1)
    expect(nudged).toMatchObject({ x: 3, y: 10 })
    await page.getByRole('button', { name: 'Undo move' }).click()
    expect(await placementOf(page, 1)).toMatchObject({ x: 0, y: 0 })
  })
})

test('two fingers tapped together undo; three redo', async ({ page }) => {
  await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
  const at = await pieceCentre(page, 0)
  await multiTap(page, at, 2)
  await expect(page.getByRole('status').filter({ hasText: 'Undo add pieces' })).toBeVisible()
  await expect(page.getByText('No pieces yet.')).toBeVisible()
  await multiTap(page, at, 3)
  await expect(page.getByRole('option', { name: /Skirt front/ })).toBeVisible()
})

test('right-click opens the piece menu; turning a piece can be undone', async ({ page }, info) => {
  test.skip(info.project.name === 'ipad-webkit', 'right-click')
  await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
  const at = await pieceCentre(page, 0)
  await page.mouse.click(at.x, at.y, { button: 'right' })
  await page.getByRole('menuitem', { name: 'Turn 90° clockwise' }).click()
  expect((await placementOf(page, 0)).rotation).toBe(90)
  await page.keyboard.press(`${await modKey(page)}+z`)
  expect((await placementOf(page, 0)).rotation).toBe(0)
})

test('the inspector renames a piece and changes its allowance', async ({ page }) => {
  await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
  await page.getByRole('option', { name: /Waistband/ }).click()
  const name = page.getByRole('textbox', { name: 'Name' })
  await name.fill('Waistband (front)')
  await name.press('Enter')
  await expect(page.getByRole('option', { name: /Waistband \(front\)/ })).toBeVisible()
  await page.getByRole('button', { name: 'Increase Seam allowance' }).click()
  await expect(page.getByRole('textbox', { name: 'Seam allowance' })).toHaveValue('1.6 cm')
})
