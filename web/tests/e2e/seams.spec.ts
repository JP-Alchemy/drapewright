import { expect, test, type Page } from './fixtures.ts'
import { edgeMiddle, fixture, openWithFixture } from './helpers.ts'

/** Picks a piece's edge as a tap on it would (through the debug handle). */
async function pickEdge(page: Page, piece: number, edge: number) {
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

/** The seam picked, and whether a suggested seam is shown on the table. */
async function shown(page: Page): Promise<{ seam: string | null; suggestion: boolean }> {
  return page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const ui = (window as any).drapewright.ui.get()
    return { seam: ui.seam, suggestion: ui.suggestion !== null }
  })
}

/** Whether a point of the page is on the table, as drawn. */
async function onTable(page: Page, at: { x: number; y: number }): Promise<boolean> {
  const r = (await page.locator('canvas').boundingBox())!
  return at.x >= r.x && at.x <= r.x + r.width && at.y >= r.y && at.y <= r.y + r.height
}

test('a seam found is shown on the table, sewn, and listed under both its pieces', async ({
  page,
}) => {
  await openWithFixture(page, 'freesewing-sasha-a0.pdf', 'Add 7 pieces')
  await expect(page.getByText(/No seams yet/)).toBeVisible()
  await page.getByRole('button', { name: 'Find seams' }).click()
  const found = page.getByRole('group', { name: 'Edges that look sewn together' })
  const suggestion = found
    .getByRole('listitem')
    .filter({ hasText: 'Back inside · edge 5 ↔ Back outside · edge 1' })

  // Tapped, a suggestion is shown on the table, its edges joined; tapped
  // again, it isn't.
  const show = suggestion.getByRole('button', { name: /↔/ })
  await show.click()
  await expect(show).toHaveAttribute('aria-pressed', 'true')
  expect((await shown(page)).suggestion).toBe(true)
  await show.click()
  await expect(show).toHaveAttribute('aria-pressed', 'false')
  expect((await shown(page)).suggestion).toBe(false)

  // Sewn, it is listed under each of its pieces: a piece chosen opens to
  // show the pieces it is sewn to, and the edges.
  const sew = found.getByRole('button', { name: 'Sew', exact: true })
  const suggested = await sew.count()
  await suggestion.getByRole('button', { name: 'Sew', exact: true }).click()
  await expect(sew).toHaveCount(suggested - 1)
  const pieces = page.getByRole('treegrid', { name: 'Pieces' })
  const back = pieces.getByRole('row', { name: 'Back inside', level: 1 })
  await back.click()
  await expect(back).toHaveAttribute('aria-expanded', 'true')
  const toOutside = pieces.getByRole('row', { name: 'Back outside, edge 5 ↔ edge 1', level: 2 })
  await expect(toOutside).toBeVisible()

  // Picked, its details show, and the table moves to show both its edges,
  // joined: from far off, where neither is.
  await page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.setViewport((v: object) => ({ ...v, cx: 9000, cy: 9000 })),
  )
  await toOutside.click()
  await expect(toOutside).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('heading', { name: 'Seam 1' })).toBeVisible()
  await expect(page.getByText('Back inside · edge 5 ↔ Back outside · edge 1')).toBeVisible()
  expect((await shown(page)).seam).not.toBeNull()
  const [inside, outside] = await page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.engine
      .document()
      .pieces.flatMap((p: { name: string }, i: number) =>
        ['Back inside', 'Back outside'].includes(p.name) ? [i] : [],
      ),
  )
  const seam = await page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => (window as any).drapewright.engine.document().seams[0],
  )
  expect(await onTable(page, await edgeMiddle(page, inside, seam.a.edge))).toBe(true)
  expect(await onTable(page, await edgeMiddle(page, outside, seam.b.edge))).toBe(true)

  // Choosing the piece it is sewn to keeps it picked, there under that one.
  await pieces.getByRole('row', { name: 'Back outside', level: 1 }).click()
  const toInside = pieces.getByRole('row', { name: 'Back inside, edge 1 ↔ edge 5', level: 2 })
  await expect(toInside).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('heading', { name: 'Seam 1' })).toBeVisible()

  // Turned round and unpicked; undo brings it back.
  const reversed = () =>
    page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).drapewright.engine.document().seams[0]?.reversed as boolean,
    )
  const before = await reversed()
  await page.getByRole('button', { name: 'Turn round' }).click()
  await expect.poll(reversed).toBe(!before)
  await page.getByRole('button', { name: 'Unpick' }).click()
  await expect(toInside).toHaveCount(0)
  await expect(page.getByRole('heading', { name: 'Seam 1' })).toHaveCount(0)
  await page.getByRole('button', { name: /^Undo/ }).click()
  await expect(toInside).toBeVisible()
})

test('a seam is picked and put down with a tap, as with a click', async ({ page }, info) => {
  const tap = (target: import('@playwright/test').Locator) =>
    info.project.name === 'ipad-webkit' ? target.tap() : target.click()
  await openWithFixture(page, 'freesewing-sasha-a0.pdf', 'Add 7 pieces')
  await tap(page.getByRole('button', { name: 'Find seams' }))
  await tap(page.getByRole('button', { name: /^Sew all/ }))
  const pieces = page.getByRole('treegrid', { name: 'Pieces' })
  await tap(pieces.getByRole('row', { name: 'Sleeve', level: 1 }))
  const underarm = pieces.getByRole('row', { name: 'Sleeve, edge 2 ↔ edge 4', level: 2 })
  await tap(underarm)
  await expect(underarm).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('heading', { name: /^Seam \d/ })).toBeVisible()
  // Again: put down, the sleeve still chosen.
  await tap(underarm)
  await expect(underarm).toHaveAttribute('aria-selected', 'false')
  expect((await shown(page)).seam).toBeNull()
  await expect(page.getByRole('textbox', { name: 'Name' })).toHaveValue('Sleeve')
  // Escape puts down the seam, and the piece.
  await tap(underarm)
  await expect(underarm).toHaveAttribute('aria-selected', 'true')
  await page.keyboard.press('Escape')
  expect((await shown(page)).seam).toBeNull()
})

test('an edge is sewn to another by hand, and the seam shown', async ({ page }) => {
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
  // Sewn and picked, on the piece tapped: under it, sewn to the first.
  await expect(page.getByRole('heading', { name: 'Seam 1' })).toBeVisible()
  const pieces = page.getByRole('treegrid', { name: 'Pieces' })
  await expect(
    pieces.getByRole('row', { name: 'Front inside, edge 2 ↔ edge 2', level: 2 }),
  ).toHaveAttribute('aria-selected', 'true')
  // The edge's details say what it is sewn to, and pick the seam again.
  await pickEdge(page, 3, 1)
  await expect(page.getByRole('heading', { name: 'Seam 1' })).toHaveCount(0)
  await page.getByRole('button', { name: /^Sewn to Front inside/ }).click()
  await expect(page.getByRole('heading', { name: 'Seam 1' })).toBeVisible()
})

test('panels drawn for one side are cut as pairs when seams are found, and it says so', async ({
  page,
}) => {
  await page.goto('/app/')
  // The test corset: three panels drawn once, from the centre front round
  // to the centre back, and a crotch piece; each cut once, as the pattern
  // leaves them.
  await page.getByTestId('pattern-file').setInputFiles(fixture('corset.drapewright'))
  const pieces = page.getByRole('treegrid', { name: 'Pieces' })
  await expect(pieces.getByRole('row', { level: 1 })).toHaveCount(4)
  const centre = pieces.getByRole('row', { name: /Front Center/, level: 1 })
  await expect(centre).toContainText('Cut 1')

  await page.getByRole('button', { name: 'Find seams' }).click()
  await expect(
    page.getByText(
      '“Front Center”, “Front Side”, and “Back Center” look drawn for one side, so they’re now cut 2, a left and a right. Undo to cut them once.',
    ),
  ).toBeVisible()
  await expect(centre).toContainText('Cut 2')
  // So each centre is sewn to its other half, and the crotch piece along
  // both centre fronts.
  const found = page.getByRole('group', { name: 'Edges that look sewn together' })
  await expect(found.getByText(/centre of a pair/)).toHaveCount(2)
  await expect(found.getByText(/fits along the opening/)).toHaveCount(2)

  // Undo cuts them once again, in one step.
  await page.getByRole('button', { name: /^Undo/ }).click()
  await expect(centre).toContainText('Cut 1')
  await expect(pieces.getByRole('row', { name: /Back Center/, level: 1 })).toContainText('Cut 1')
})
