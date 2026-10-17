import { expect, type Page, test } from './fixtures.ts'
import { openBackup } from './helpers.ts'

/** Opens the avatar; whether this browser draws it in 3D (not WebKit on Linux, as in CI). */
async function openTheAvatar(page: Page): Promise<boolean> {
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drawnWith = pane.getByText(/^Drawn with (WebGPU|WebGL2)$/)
  const cannot = pane.getByText('This browser can’t draw the avatar in 3D.')
  await expect(drawnWith.or(cannot)).toBeVisible({ timeout: 60_000 })
  return drawnWith.isVisible()
}

interface Seen {
  piece: string
  edge: string
  points: number[]
  shown: boolean[]
  middle: number
}

/** The page point of a piece edge's middle as seen on the figure, on a copy facing the eye. */
function seenMiddle(page: Page, piece: string, edge: string) {
  return page.waitForFunction(
    ({ piece, edge }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const figure = (window as any).drapewrightFigure
      const seen: Seen[] | null = figure?.seen() ?? null
      const e = seen?.find((s) => s.piece === piece && s.edge === edge && s.shown[s.middle])
      if (!e) return null
      const r = document.querySelector('.avatar-view')!.getBoundingClientRect()
      return {
        x: r.left + e.points[2 * e.middle]!,
        y: r.top + e.points[2 * e.middle + 1]!,
      }
    },
    { piece, edge },
  )
}

/** The page point of a handle shown on the figure. */
function handleOnFigure(page: Page, kind: 'extend' | 'extrude') {
  return page.evaluate((kind) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const handles = (window as any).drapewrightFigure?.handles()
    const r = document.querySelector('.avatar-view')!.getBoundingClientRect()
    return handles ? { x: r.left + handles[kind].x, y: r.top + handles[kind].y } : null
  }, kind)
}

const ui = (page: Page) =>
  page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { hover, part } = (window as any).drapewright.ui.get()
    return { hover: hover && [hover.piece, hover.edge], part: part && [part.piece, part.edge] }
  })

test('an edge is hovered, chosen and dragged out on the figure', async ({ page }, info) => {
  test.setTimeout(240_000)
  test.skip(info.project.name === 'ipad-webkit', 'mouse hover and drag')
  await page.goto('/app/?debug')
  // The test skirt: its front (p1) cut on the fold, its waist edge e2.
  await openBackup(page, 'skirt.drapewright', 2)
  test.skip(!(await openTheAvatar(page)), 'Editing on the figure is in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  const editing = drape.getByRole('button', { name: 'Edit on the figure' })
  await expect(editing).toBeDisabled()
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  await expect(pane.getByRole('status').last()).toHaveText('Draped in poplin.', {
    timeout: 120_000,
  })
  await editing.click()
  await expect(pane.getByText(/Hover or tap an edge/)).toBeVisible()

  // Hovered on the figure: hovered on the table too.
  const waist = (await (await seenMiddle(page, 'p1', 'e2')).jsonValue())!
  await page.mouse.move(waist.x, waist.y + 30)
  await page.mouse.move(waist.x, waist.y, { steps: 4 })
  await expect.poll(async () => (await ui(page)).hover).toEqual(['p1', 'e2'])
  // Tapped: chosen, its details in the side panel.
  await page.mouse.click(waist.x, waist.y)
  await expect.poll(async () => (await ui(page)).part).toEqual(['p1', 'e2'])
  await expect(page.getByText(/drag the handles beside its length/)).toBeVisible()

  // Its extend handle dragged up: the front longer, as one step.
  const handle = (await handleOnFigure(page, 'extend'))!
  expect(handle).not.toBeNull()
  await page.mouse.move(handle.x, handle.y, { steps: 4 })
  await page.mouse.down()
  await page.mouse.move(handle.x, handle.y - 40, { steps: 8 })
  await page.mouse.up()
  const steps = await page.evaluate(() =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).drapewright.engine
      .document()
      .pieces[0].alterations.map(
        (a: { change: { type: string; edge: string; by: number; mode: string } }) => a.change,
      ),
  )
  expect(steps).toHaveLength(1)
  expect(steps[0]).toMatchObject({ type: 'moveEdge', edge: 'e2', mode: 'extend' })
  expect(steps[0].by).toBeGreaterThan(5)
  await expect(page.getByRole('switch', { name: /^Edge 1 extended/ }).first()).toBeChecked()
  // The garment drapes again, following the pattern.
  await expect(pane.getByRole('status').last()).toHaveText('Draped in poplin.', {
    timeout: 120_000,
  })
})
