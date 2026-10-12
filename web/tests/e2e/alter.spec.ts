import { expect, type Page, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

/** Adds a step of the kind named, from the inspector's menu. */
async function addStep(page: Page, kind: string) {
  await page.getByRole('button', { name: 'Add a step' }).click()
  await page.getByRole('menuitem', { name: new RegExp(`^${kind}`) }).click()
}

/** Where a point of piece `index` (its own mm) is on the page. */
function pagePoint(page: Page, index: number, x: number, y: number) {
  return page.evaluate(
    ([index, x, y]) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      const piece = s.engine.document().pieces[index]
      const v = s.ui.get().viewport
      const scale = v.pxPerMm * v.zoom
      const r = document.querySelector('canvas')!.getBoundingClientRect()
      return {
        x: r.left + (x + piece.placement.x - v.cx) * scale + v.width / 2,
        y: r.top + (y + piece.placement.y - v.cy) * scale + v.height / 2,
      }
    },
    [index, x, y] as const,
  )
}

/** A point `t` of the way along the first curve of edge `edge` of piece `index`. */
function onCurve(page: Page, index: number, edge: number, t: number) {
  return page.evaluate(
    ([index, edge, t]) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      const edges = s.engine.document().pieces[index].outline.edges
      const p0 = edges[edge - 1].segments.at(-1).to
      const seg = edges[edge].segments[0]
      const [c1, c2] = seg.c1 ? [seg.c1, seg.c2] : [p0, seg.to]
      const u = 1 - t
      const at = (k: number) =>
        u * u * u * p0[k] + 3 * u * u * t * c1[k] + 3 * u * t * t * c2[k] + t * t * t * seg.to[k]
      return [at(0), at(1)] as [number, number]
    },
    [index, edge, t] as const,
  )
}

async function tapOn(page: Page, index: number, [x, y]: [number, number]) {
  const p = await pagePoint(page, index, x, y)
  await page.mouse.click(p.x, p.y)
}

test('a piece is lengthened, switched off, shortened instead, and as drawn again', async ({
  page,
}) => {
  await page.goto('/')
  // The test blouse: its front and back 60 cm long, sewn at the sides.
  await page.getByTestId('pattern-file').setInputFiles(fixture('blouse.drapewright'))
  const pieces = page.getByRole('listbox', { name: 'Pieces' })
  const front = pieces.getByRole('option', { name: /Blouse front/ })
  await expect(front).toContainText('25.5 cm × 60.0 cm')
  await front.click()

  // Lengthened 1 cm across its middle: the side seam no longer matches.
  await addStep(page, 'Lengthen or shorten')
  await expect(front).toContainText('25.5 cm × 61.0 cm')
  const step = page.getByRole('switch', { name: 'Lengthened by 1.0 cm' })
  await expect(step).toBeChecked()
  await expect(page.getByText(/the first side 1\.0 cm longer/).first()).toBeVisible()

  // Switched off, the piece is as drawn; on again, lengthened.
  const label = page.getByText('Lengthened by 1.0 cm', { exact: true })
  await label.click()
  await expect(step).not.toBeChecked()
  await expect(front).toContainText('25.5 cm × 60.0 cm')
  await label.click()
  await expect(front).toContainText('25.5 cm × 61.0 cm')

  // Shortened by 2 cm instead.
  await page.getByText('Shorten', { exact: true }).click()
  await expect(front).toContainText('25.5 cm × 59.0 cm')
  const by = page.getByRole('textbox', { name: 'By how much' })
  await by.fill('2')
  await by.press('Enter')
  await expect(front).toContainText('25.5 cm × 58.0 cm')
  await expect(page.getByRole('switch', { name: 'Shortened by 2.0 cm' })).toBeChecked()

  // Taken away: as drawn again; undone, shortened again.
  await page.getByRole('button', { name: 'Remove the step “Shortened by 2.0 cm”' }).click()
  await expect(front).toContainText('25.5 cm × 60.0 cm')
  await expect(page.getByText(/None yet\. Each change is kept as a step/)).toBeVisible()
  await page.getByRole('button', { name: 'Undo remove alteration' }).click()
  await expect(front).toContainText('25.5 cm × 58.0 cm')
})

test('a bust adjustment is placed by tapping the bust point, armhole and side seam', async ({
  page,
}) => {
  await page.goto('/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('blouse.drapewright'))
  const front = page.getByRole('listbox', { name: 'Pieces' }).getByRole('option', {
    name: /Blouse front/,
  })
  await front.click()
  /** Where a point of the front (its own mm) is on the page. */
  const at = (x: number, y: number) =>
    page.evaluate(
      ([x, y]) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const s = (window as any).drapewright
        const piece = s.engine.document().pieces[0]
        const v = s.ui.get().viewport
        const scale = v.pxPerMm * v.zoom
        const r = document.querySelector('canvas')!.getBoundingClientRect()
        return {
          x: r.left + (x + piece.placement.x - v.cx) * scale + v.width / 2,
          y: r.top + (y + piece.placement.y - v.cy) * scale + v.height / 2,
        }
      },
      [x, y] as const,
    )
  const tap = async (x: number, y: number) => {
    const p = await at(x, y)
    await page.mouse.click(p.x, p.y)
  }

  // Started, then cancelled: nothing changes.
  await addStep(page, 'Bust adjustment')
  await expect(page.getByText('Tap the bust point.')).toBeVisible()
  await page.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByText('Tap the bust point.')).toHaveCount(0)

  await addStep(page, 'Bust adjustment')
  await tap(90, 250)
  await expect(page.getByText(/Tap the armhole where it hinges/)).toBeVisible()
  // On the armhole: snapped to it.
  await tap(193.4, 170.8)
  await expect(page.getByText('Tap the side seam where the dart goes.')).toBeVisible()
  await tap(251.1, 300)
  await expect(page.getByRole('switch', { name: 'Full bust adjustment, 2.0 cm' })).toBeChecked()
  // Wider across the bust and longer at the centre front.
  await expect(front).not.toContainText('25.5 cm × 60.0 cm')
  await expect(front).toContainText(/2[6-9]\.\d cm × 6[1-3]\.\d cm/)
  // The side seam measured with the new dart closed: as long as the back's.
  await expect(page.getByText(/^Same length, 38\.\d cm/)).toBeVisible()
  // Its dart pivoted to the armhole: the piece's only dart, so only where
  // it goes is tapped (halfway up the armhole's part above the hinge).
  const document = () =>
    page.evaluate(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      return s.engine.document().pieces[0]
    })
  const { armhole, side, armholeAt } = await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const edges = s.engine.document().pieces[0].outline.edges
    const [p0, { c1, c2, to: p3 }] = [edges[1].segments.at(-1).to, edges[2].segments[0]]
    const mid = (k: number) => (p0[k] + 3 * c1[k] + 3 * c2[k] + p3[k]) / 8
    return {
      armhole: edges[2].id as string,
      side: edges[3].id as string,
      armholeAt: [mid(0), mid(1)] as [number, number],
    }
  })
  await addStep(page, 'Move a dart')
  await expect(page.getByText('Tap the edge where the dart goes.')).toBeVisible()
  await tap(...armholeAt)
  await expect(page.getByRole('switch', { name: 'Dart moved' })).toBeChecked()
  let darts = (await document()).darts
  expect(darts.map((d: { edge: string }) => d.edge)).toEqual([armhole])
  // Half of it: half stays in the side seam.
  await page.getByText('Half', { exact: true }).click()
  await expect(page.getByRole('switch', { name: 'Half a dart moved' })).toBeChecked()
  darts = (await document()).darts
  expect(darts.map((d: { edge: string }) => d.edge).sort()).toEqual([armhole, side].sort())
  await page.getByRole('button', { name: 'Remove the step “Half a dart moved”' }).click()

  // A small bust adjustment instead.
  await page.getByText('Small bust', { exact: true }).click()
  await expect(page.getByRole('switch', { name: 'Small bust adjustment, 2.0 cm' })).toBeChecked()
  await expect(front).toContainText(/2[2-5]\.\d cm × 5[5-9]\.\d cm/)
})

test('shoulders are made broader by tapping the shoulder seam and the armhole', async ({
  page,
}) => {
  await page.goto('/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('blouse.drapewright'))
  await page
    .getByRole('listbox', { name: 'Pieces' })
    .getByRole('option', { name: /Blouse front/ })
    .click()
  // The shoulder in the middle of the table, clear of the prompt at its top left.
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    s.setViewport((v: object) => ({ ...v, cx: 150, cy: 80 }))
  })
  await addStep(page, 'Shoulders')
  await expect(page.getByText('Tap the shoulder seam about halfway along.')).toBeVisible()
  // Halfway along the shoulder seam, (70, 0) to (174, 42); halfway down the armhole.
  await tapOn(page, 0, [122, 21])
  await tapOn(page, 0, await onCurve(page, 0, 2, 0.5))
  await expect(page.getByRole('switch', { name: 'Shoulders 1.0 cm broader' })).toBeChecked()
  // The shoulder seam's end 1 cm further out along it.
  const end = await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    return s.engine.document().pieces[0].outline.edges[1].segments.at(-1).to as [number, number]
  })
  const along = ((end[0] - 174) * 104 + (end[1] - 42) * 42) / Math.hypot(104, 42)
  expect(along).toBeCloseTo(10, 0)

  // The shoulder seam, jogged where it was slashed, trued straight.
  const shoulder = () =>
    page.evaluate(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      return s.engine.document().pieces[0].outline.edges[1].segments.length as number
    })
  expect(await shoulder()).toBeGreaterThan(1)
  await addStep(page, 'True up an edge')
  await expect(page.getByText('Tap the edge to true.')).toBeVisible()
  await tapOn(page, 0, [70 + 0.3 * 104, 0.3 * 42])
  await expect(page.getByRole('switch', { name: /^Edge 2 trued (straight|smooth)$/ })).toBeChecked()
  await page.getByText('Straight', { exact: true }).click()
  await expect(page.getByRole('switch', { name: 'Edge 2 trued straight' })).toBeChecked()
  expect(await shoulder()).toBe(1)
})

test('a swayback and a waist are adjusted on a skirt', async ({ page }) => {
  await page.goto('/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  const pieces = page.getByRole('listbox', { name: 'Pieces' })
  // The back: its centre back a seam, (0, 616) up to (0, 0).
  await pieces.getByRole('option', { name: /Skirt back/ }).click()
  await addStep(page, 'Swayback')
  await expect(page.getByText('Tap the centre back where the wedge is taken out.')).toBeVisible()
  const side = await onCurve(page, 1, 1, 0.15)
  await tapOn(page, 1, [0, side[1]])
  await tapOn(page, 1, side)
  await expect(page.getByRole('switch', { name: 'Swayback, 1.5 cm taken out' })).toBeChecked()
  // A front cut on the fold: let out at the side seam, tapering to the hip.
  await pieces.getByRole('option', { name: /Skirt front/ }).click()
  await addStep(page, 'Waist')
  await tapOn(page, 0, await onCurve(page, 0, 1, 0.02))
  await tapOn(page, 0, await onCurve(page, 0, 1, 0.3))
  await expect(page.getByRole('switch', { name: 'Waist let out 1.0 cm at the side' })).toBeChecked()
  const waistEnd = await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    return s.engine.document().pieces[0].outline.edges[0].segments.at(-1).to as [number, number]
  })
  expect(waistEnd[0]).toBeGreaterThan(193)
})
