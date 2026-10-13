import { expect, test } from './fixtures.ts'
import { fixture, modKey, multiTap, openWithFixture, pieceCentre, placementOf } from './helpers.ts'

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

test('several pieces are chosen with Select and cut at once', async ({ page }, info) => {
  const tap = (target: import('@playwright/test').Locator) =>
    info.project.name === 'ipad-webkit' ? target.tap() : target.click()
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('corset.drapewright'))
  const pieces = page.getByRole('listbox', { name: 'Pieces' })
  await expect(pieces.getByRole('option')).toHaveCount(4)

  // Select turns tapping into ticking, for a finger as for a mouse.
  await tap(page.getByRole('button', { name: 'Select several pieces' }))
  for (const name of [/Front Center/, /Front Side/, /Back Center/]) {
    await tap(pieces.getByRole('option', { name }))
  }
  await expect(page.getByRole('heading', { name: '3 pieces' })).toBeVisible()
  await expect(
    page.getByText(/apply to all of them: “Front Center”, “Front Side”, and/),
  ).toBeVisible()

  // One press cuts all three twice, as mirrored pairs; one undo, once again.
  await tap(page.getByRole('button', { name: 'Increase How many' }))
  for (const name of [/Front Center/, /Front Side/, /Back Center/]) {
    await expect(pieces.getByRole('option', { name })).toContainText('Cut 2')
  }
  await expect(pieces.getByRole('option', { name: /Crotch Front/ })).toContainText('Cut 1')
  await expect(page.getByRole('switch', { name: 'As mirrored pairs' })).toBeChecked()
  await page.getByRole('button', { name: /^Undo/ }).click()
  await expect(pieces.getByRole('option', { name: /Front Side/ })).toContainText('Cut 1')

  // Pieces that differ show it, and a press sets each from its own.
  await tap(pieces.getByRole('option', { name: /Front Side/ }))
  await expect(page.getByRole('heading', { name: '2 pieces' })).toBeVisible()
  await tap(page.getByRole('button', { name: 'Increase How many' }))
  await tap(pieces.getByRole('option', { name: /Front Side/ }))
  await expect(page.getByRole('textbox', { name: 'How many' })).toHaveAttribute(
    'placeholder',
    'Mixed',
  )
  await tap(page.getByRole('button', { name: 'Increase How many' }))
  await expect(pieces.getByRole('option', { name: /Front Center/ })).toContainText('Cut 3')
  await expect(pieces.getByRole('option', { name: /Front Side/ })).toContainText('Cut 2')

  // Done: tapping chooses one piece again.
  await tap(page.getByRole('button', { name: 'Done' }))
  await tap(pieces.getByRole('option', { name: /Crotch Front/ }))
  await expect(page.getByRole('heading', { name: 'Piece', exact: true })).toBeVisible()
  await expect(page.getByRole('textbox', { name: 'Name' })).toHaveValue('Crotch Front')
})

test('a part of the details folds away, says what it holds, and stays folded', async ({ page }) => {
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('corset.drapewright'))
  await page.getByRole('option', { name: /Front Side/ }).click()
  const allowance = page.getByRole('button', { name: /^Allowance/ })
  await expect(allowance).toHaveAttribute('aria-expanded', 'true')
  await allowance.click()
  await expect(allowance).toHaveAttribute('aria-expanded', 'false')
  await expect(allowance).toContainText('Added · 1.0 cm')
  await expect(page.getByRole('textbox', { name: 'Seam allowance' })).toBeHidden()

  // Another piece, and after a reload: still folded.
  await page.getByRole('option', { name: /Back Center/ }).click()
  await expect(allowance).toHaveAttribute('aria-expanded', 'false')
  await page.reload()
  await page.getByRole('option', { name: /Front Center/ }).click()
  await expect(page.getByRole('button', { name: /^Allowance/ })).toHaveAttribute(
    'aria-expanded',
    'false',
  )
})

test('the bar between the lists and the details moves', async ({ page }) => {
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('corset.drapewright'))
  // Opened: the panel is the project's own.
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(4)
  const bar = page.getByRole('separator', { name: 'Resize the lists and the details' })
  await expect(bar).toHaveAttribute('aria-valuenow', '45')
  await bar.focus()
  await page.keyboard.press('ArrowDown')
  await page.keyboard.press('ArrowDown')
  await expect(bar).toHaveAttribute('aria-valuenow', '55')
  await page.keyboard.press('Home')
  await expect(bar).toHaveAttribute('aria-valuenow', '15')
})
