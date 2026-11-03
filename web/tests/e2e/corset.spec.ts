import { expect, type Page, test } from './fixtures.ts'
import { openWithFixture } from './helpers.ts'

// The test corset (fixtures/pdf/corset-boning.pdf): a centre front with a
// boning channel, a side panel boned up its right seam to a "Stop boning
// here" mark, and a centre back with eyelets down it between two lacing
// bones; each cut 2.

/** The project as it is: its seams' lacing, and each piece's boning lines by id. */
const project = (page: Page) =>
  page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const doc = (window as any).drapewright.engine.document()
    return {
      lacing: doc.seams.map((s: { lacing: number | null }) => s.lacing),
      bones: Object.fromEntries(
        doc.pieces.map((p: { name: string; lines: { id: string; kind: string }[] }) => [
          p.name,
          p.lines.filter((l) => l.kind === 'boning').map((l) => l.id),
        ]),
      ),
    }
  })

test('a corset’s back is laced where eyelets are marked, and its seams boned', async ({ page }) => {
  await openWithFixture(page, 'corset-boning.pdf', 'Add 3 pieces')
  // Read from the pattern: the front's channel, the side's bone up its
  // seam, the back's two lacing bones.
  const read = await project(page)
  expect(read.bones['Front center']).toHaveLength(2)
  expect(read.bones['Front side']).toHaveLength(1)
  expect(read.bones['Back center']).toHaveLength(4)

  // Find seams laces the back: eyelets are marked along it.
  await page.getByRole('button', { name: 'Find seams' }).click()
  const suggestions = page.getByRole('group', { name: 'Edges that look sewn together' })
  const laced = suggestions.getByRole('listitem').filter({ hasText: 'eyelets along it' })
  await expect(laced).toHaveCount(1)
  await expect(laced.getByRole('button', { name: 'Lace', exact: true })).toBeVisible()
  await page.getByRole('button', { name: /^Sew all/ }).click()
  expect((await project(page)).lacing.filter((l: number | null) => l !== null)).toEqual([50])

  // The laced seam's details: left open by 5 cm, opened wider.
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const seam = s.engine.document().seams.find((x: { lacing: number | null }) => x.lacing)
    s.selectSeam(seam.id, seam.a.piece)
  })
  await expect(page.getByRole('switch', { name: 'Laced, not sewn' })).toBeChecked()
  const gap = page.getByRole('textbox', { name: 'Left open by' })
  await expect(gap).toHaveValue('5 cm')
  await gap.fill('7')
  await gap.press('Tab')
  await expect
    .poll(async () => (await project(page)).lacing.filter((l: number | null) => l !== null))
    .toEqual([70])

  // The front's seam boned by hand, and the bone taken out again.
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const front = s.engine
      .document()
      .pieces.find((p: { name: string }) => p.name === 'Front center')
    const edge = front.outline.edges[1].id
    s.select([front.id], { kind: 'edge', piece: front.id, edge })
  })
  const boned = page.getByRole('switch', { name: 'Boned: a bone sewn along it' })
  await expect(boned).not.toBeChecked()
  await page.getByText('Boned: a bone sewn along it').click()
  await expect(boned).toBeChecked()
  expect((await project(page)).bones['Front center']).toHaveLength(3)
  await page.getByText('Boned: a bone sewn along it').click()
  await expect(boned).not.toBeChecked()
  expect((await project(page)).bones['Front center']).toHaveLength(2)
})
