import { expect, test } from '@playwright/test'
import { edgeMiddle, edgesOf, modKey, openWithFixture } from './helpers.ts'

test('marking a hem and choosing a corner shape', async ({ page }) => {
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  // Edges of the flared skirt: e2 waist, e3 side seam, e4 hem, e5 centre front.
  const hem = await edgeMiddle(page, 0, 'e4')
  await page.mouse.click(hem.x, hem.y)
  await page
    .getByRole('radiogroup', { name: 'This edge is a' })
    .getByText('Hem', { exact: true })
    .click()
  expect((await edgesOf(page, 0))[2]).toEqual(['e4', 'hem', 'auto', null])

  // The corner where the side seam meets the hem.
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).soom
    const p = s.engine.document().pieces[0]
    s.select([p.id], { kind: 'corner', piece: p.id, edge: 'e3' })
  })
  const chooser = page.getByRole('radiogroup', { name: 'Shape of the allowance here' })
  await expect(chooser).toBeVisible()
  await chooser.getByText('Pointed', { exact: true }).click()
  expect((await edgesOf(page, 0))[1]).toEqual(['e3', 'seam', 'mitre', null])
  await page.keyboard.press(`${await modKey(page)}+z`)
  expect((await edgesOf(page, 0))[1]).toEqual(['e3', 'seam', 'auto', null])
})

test('adding and removing a corner from the menu', async ({ page }, info) => {
  test.skip(info.project.name === 'ipad-webkit', 'right-click')
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  const side = await edgeMiddle(page, 0, 'e3')
  await page.mouse.click(side.x, side.y, { button: 'right' })
  await page.getByRole('menuitem', { name: 'Add a corner here' }).click()
  expect(await edgesOf(page, 0)).toHaveLength(5)
  await page.getByRole('button', { name: 'Undo add corner' }).click()
  expect(await edgesOf(page, 0)).toHaveLength(4)
})

test('an edge can have its own allowance, and go back to the piece’s', async ({ page }) => {
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  const waist = await edgeMiddle(page, 0, 'e2')
  await page.mouse.click(waist.x, waist.y)
  await page.getByRole('button', { name: 'Increase Allowance on this edge' }).click()
  expect((await edgesOf(page, 0))[0]?.[3]).toBeCloseTo(16, 6)
  await page.getByRole('button', { name: 'Use the piece’s allowance' }).click()
  expect((await edgesOf(page, 0))[0]?.[3]).toBeNull()
})

test('a curved edge cannot go on the fold, and Soom says why', async ({ page }) => {
  await openWithFixture(page, 'inkscape-mm.svg', 'Add 2 pieces')
  // The skirt's hem (e4) is curved.
  const hem = await edgeMiddle(page, 0, 'e4')
  await page.mouse.click(hem.x, hem.y)
  await page
    .getByRole('radiogroup', { name: 'This edge is a' })
    .getByText('Fold', { exact: true })
    .click()
  await expect(page.getByRole('alert')).toContainText('not straight')
  expect((await edgesOf(page, 0))[2]?.[1]).toBe('seam')
})
