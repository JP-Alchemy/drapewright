import { expect, type Page, test } from './fixtures.ts'
import { edgeMiddle, fingerDrag, openBackup } from './helpers.ts'

// The test blouse's front: 25.5 cm across and 60 cm long, cut on the fold
// down its left side (edge e7), its hem (e6) along the bottom, its side
// seam (e5) flaring 5 mm on the way down.

/**
 * Where a handle beside an edge's length is on the page, laid out as the
 * table lays it out: the length's label just beyond the allowance, a
 * handle either side of it (extend on the left or above, extrude on the
 * right or below). For an unturned piece, lengths in cm.
 */
function handleOf(page: Page, index: number, edge: string, kind: 'extend' | 'extrude') {
  return page.evaluate(
    ({ index, edge, kind }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      const v = s.ui.get().viewport
      const sp = s.scene()[index]
      const ev = sp.view.edges.find((e: { id: string }) => e.id === edge)
      const scale = v.pxPerMm * v.zoom
      const r = document.querySelector('canvas')!.getBoundingClientRect()
      const reach = ev.allowance + 14 / scale
      const [mx, my] = ev.middle
      const [nx, ny] = ev.middleNormal
      const pill = {
        x: r.left + (mx + nx * reach + sp.piece.placement.x - v.cx) * scale + v.width / 2,
        y: r.top + (my + ny * reach + sp.piece.placement.y - v.cy) * scale + v.height / 2,
      }
      const measure = document.createElement('canvas').getContext('2d')!
      measure.font = '600 12px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
      const width = measure.measureText(`${(ev.length / 10).toFixed(1)} cm`).width + 14
      const along = { x: -ny, y: nx }
      const away = (Math.abs(along.x) * width) / 2 + (Math.abs(along.y) * 22) / 2 + 6 + 13
      const a = { x: pill.x - along.x * away, y: pill.y - along.y * away }
      const b = { x: pill.x + along.x * away, y: pill.y + along.y * away }
      const aFirst = a.x < b.x - 0.5 || (Math.abs(a.x - b.x) <= 0.5 && a.y < b.y)
      return (kind === 'extend') === aFirst ? a : b
    },
    { index, edge, kind },
  )
}

/** Screen px a millimetre on the table is, at the zoom shown. */
const pxPerMm = (page: Page) =>
  page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const v = (window as any).drapewright.ui.get().viewport
    return v.pxPerMm * v.zoom
  })

/**
 * The view on the front at 25%, clear of the controls over the table's
 * corners, and near enough that a drag of a centimetre or two is past a
 * finger's slop.
 */
const centreOnFront = (page: Page) =>
  page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const sp = s.scene()[0]
    const [x0, y0, x1, y1] = sp.view.bounds
    const cx = (x0 + x1) / 2 + sp.piece.placement.x
    // A little below its middle: room for its hem to move down.
    const cy = (y0 + y1) / 2 + sp.piece.placement.y + 100
    s.setViewport((v: object) => ({ ...v, cx, cy, zoom: 0.25 }))
  })

/** Hovers an edge, then drags one of its handles `dx`, `dy` mm. */
async function dragHandle(
  page: Page,
  edge: string,
  kind: 'extend' | 'extrude',
  dx: number,
  dy: number,
) {
  const middle = await edgeMiddle(page, 0, edge)
  await page.mouse.move(middle.x, middle.y)
  const at = await handleOf(page, 0, edge, kind)
  // On the way out to it, the edge stays shown.
  await page.mouse.move(at.x, at.y, { steps: 6 })
  const mm = await pxPerMm(page)
  await page.mouse.down()
  await page.mouse.move(at.x + dx * mm, at.y + dy * mm, { steps: 8 })
  await page.mouse.up()
}

test('an edge is dragged out by its handles, extended or extruded, as one undo step each', async ({
  page,
}, info) => {
  test.skip(info.project.name === 'ipad-webkit', 'mouse drag')
  await page.goto('/app/?debug')
  await openBackup(page, 'blouse.drapewright', 3)
  const front = page
    .getByRole('treegrid', { name: 'Pieces' })
    .getByRole('row', { name: /Blouse front/, level: 1 })
  await expect(front).toContainText('25.5 cm × 60.0 cm')
  await centreOnFront(page)

  // The hem dragged 3 cm down by its extend handle: the front 3 cm longer.
  await dragHandle(page, 'e6', 'extend', 0, 30.3)
  await expect(front).toContainText('25.5 cm × 63.0 cm')
  await expect(page.getByRole('switch', { name: 'The hem extended 3.0 cm' }).first()).toBeChecked()

  // Dragged back up 1 cm: the same step, now 2 cm.
  await dragHandle(page, 'e6', 'extend', 0, -10.3)
  await expect(front).toContainText('25.5 cm × 62.0 cm')
  const steps = () =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    page.evaluate(() => (window as any).drapewright.engine.document().pieces[0].alterations.length)
  expect(await steps()).toBe(1)
  await expect(page.getByRole('switch', { name: 'The hem extended 2.0 cm' }).first()).toBeChecked()

  // The fold extruded 2 cm out to the left: 2 cm wider, the neckline and
  // hem carried straight out to meet it.
  await dragHandle(page, 'e7', 'extrude', -20.3, 0)
  await expect(front).toContainText('27.5 cm × 62.0 cm')
  expect(await steps()).toBe(2)
  await expect(page.getByRole('switch', { name: 'The fold extruded 2.0 cm' }).first()).toBeChecked()

  // One undo takes the whole drag back.
  await page.getByRole('button', { name: 'Undo alter piece' }).click()
  await expect(front).toContainText('25.5 cm × 62.0 cm')
})

test('a chosen edge is dragged by a finger on its handle', async ({ page }) => {
  await page.goto('/app/?debug')
  await openBackup(page, 'blouse.drapewright', 3)
  const front = page
    .getByRole('treegrid', { name: 'Pieces' })
    .getByRole('row', { name: /Blouse front/, level: 1 })
  await centreOnFront(page)
  // Chosen by a tap, the hem shows its handles; no pointer hovers.
  const middle = await edgeMiddle(page, 0, 'e6')
  await page.evaluate((at) => {
    const canvas = document.querySelector('canvas')!
    for (const type of ['pointerdown', 'pointerup']) {
      canvas.dispatchEvent(
        new PointerEvent(type, {
          bubbles: true,
          pointerId: 300,
          pointerType: 'touch',
          isPrimary: true,
          clientX: at.x,
          clientY: at.y,
        }),
      )
    }
  }, middle)
  await expect(page.getByText(/drag the handles beside its length/)).toBeVisible()
  const at = await handleOf(page, 0, 'e6', 'extrude')
  const mm = await pxPerMm(page)
  await fingerDrag(page, at, { x: at.x, y: at.y + 20.3 * mm })
  await expect(front).toContainText('25.5 cm × 62.0 cm')
  await expect(page.getByRole('switch', { name: 'The hem extruded 2.0 cm' }).first()).toBeChecked()
})
