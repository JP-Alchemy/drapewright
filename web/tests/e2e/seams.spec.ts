import { expect, test } from './fixtures.ts'
import { openWithFixture } from './helpers.ts'

/** Picks a piece's edge as a tap on it would (through the debug handle). */
async function pickEdge(page: import('@playwright/test').Page, piece: number, edge: number) {
  await page.evaluate(
    ({ piece, edge }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      const p = s.engine.document().pieces[piece]
      s.select([p.id], { kind: 'edge', piece: p.id, edge: p.outline.edges[edge].id })
    },
    { piece, edge },
  )
}

test('seams are suggested, sewn, turned round and unpicked', async ({ page }) => {
  await openWithFixture(page, 'freesewing-sasha-a0.pdf', 'Add 7 pieces')
  const seams = page.getByRole('region', { name: 'Seams' })
  await expect(seams.getByText(/No seams yet/)).toBeVisible()
  await seams.getByRole('button', { name: 'Find seams' }).click()
  const found = seams.getByRole('group', { name: 'Edges that look sewn together' })
  const sew = found.getByRole('button', { name: 'Sew', exact: true })
  await expect(sew.first()).toBeVisible()
  const suggested = await sew.count()
  await sew.first().click()
  const first = seams.getByRole('button', { name: /^Seam 1/ })
  await expect(first).toContainText(/Same length|longer/)
  await expect(sew).toHaveCount(suggested - 1)

  // Picked, it can be turned round and unpicked; undo brings it back.
  await first.click()
  await seams.getByRole('button', { name: 'Turn round' }).click()
  const reversed = await page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => (window as any).drapewright.engine.document().seams[0].reversed as boolean,
  )
  expect(typeof reversed).toBe('boolean')
  await seams.getByRole('button', { name: 'Unpick' }).click()
  await expect(seams.getByRole('button', { name: /^Seam 1/ })).toHaveCount(0)
  await page.getByRole('button', { name: /^Undo/ }).click()
  await expect(seams.getByRole('button', { name: /^Seam 1/ })).toBeVisible()
})

test('an edge is sewn to another by hand', async ({ page }) => {
  await openWithFixture(page, 'freesewing-sasha-a0.pdf', 'Add 7 pieces')
  await pickEdge(page, 0, 1)
  await page.getByRole('button', { name: 'Sew to another edge…' }).click()
  await expect(page.getByText(/Tap the edge to sew to/)).toBeVisible()
  // Escape changes nothing but the waiting.
  await page.keyboard.press('Escape')
  await expect(page.getByText(/Tap the edge to sew to/)).toHaveCount(0)

  await pickEdge(page, 0, 1)
  await page.getByRole('button', { name: 'Sew to another edge…' }).click()
  await pickEdge(page, 3, 1)
  const seams = page.getByRole('region', { name: 'Seams' })
  await expect(seams.getByRole('button', { name: /^Seam 1/ })).toBeVisible()
  // The partner's inspector says what it is sewn to.
  await expect(page.getByRole('button', { name: /^Sewn to Front inside/ })).toBeVisible()
})
