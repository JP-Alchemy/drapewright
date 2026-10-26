import { fileURLToPath } from 'node:url'
import { expect, type Locator, type Page } from '@playwright/test'

/** A file in `fixtures/svg`, `fixtures/dxf` or `fixtures/pdf`, by its extension. */
export const fixture = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/${name.split('.').pop()}/${name}`, import.meta.url))

/**
 * Opens a backup in `fixtures/drapewright` as the project, and waits for
 * its `pieces` pieces. A project opened replaces the one shown, closing
 * whatever was opened while it was read (the avatar, the projector): on a
 * slow machine, the restore can take a second.
 */
export async function openBackup(page: Page, name: string, pieces: number): Promise<void> {
  await page.getByTestId('pattern-file').setInputFiles(fixture(name))
  await expect(
    page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', { level: 1 }),
  ).toHaveCount(pieces)
}

/**
 * Opens the app with the debug handle and imports a fixture. When the
 * import asks whether seam allowance is included, `allowance` answers.
 */
export async function openWithFixture(
  page: Page,
  name: string,
  add: string,
  allowance: 'Not included' | 'Already included' = 'Not included',
): Promise<void> {
  await page.goto('/app/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture(name))
  const dialog = page.getByRole('dialog')
  const addButton = dialog.getByRole('button', { name: add })
  await addButton.waitFor()
  const question = dialog.getByRole('radiogroup', { name: /seam allowance included/ })
  if ((await question.count()) > 0) await question.getByText(allowance, { exact: true }).click()
  await addButton.click()
  // The dialog closes once the pieces are in the document.
  await dialog.waitFor({ state: 'hidden' })
}

/** Page coordinates of the middle of a piece, by index. */
export async function pieceCentre(page: Page, index: number): Promise<{ x: number; y: number }> {
  return page.evaluate((i) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const v = s.ui.get().viewport
    const sp = s.scene()[i]
    const [x0, y0, x1, y1] = sp.view.bounds
    const scale = v.pxPerMm * v.zoom
    const r = document.querySelector('canvas')!.getBoundingClientRect()
    return {
      x: r.left + ((x0 + x1) / 2 + sp.piece.placement.x - v.cx) * scale + v.width / 2,
      y: r.top + ((y0 + y1) / 2 + sp.piece.placement.y - v.cy) * scale + v.height / 2,
    }
  }, index)
}

/** Page coordinates of a point on the work table, mm. */
export async function tablePoint(
  page: Page,
  x: number,
  y: number,
): Promise<{ x: number; y: number }> {
  return page.evaluate(
    ({ x, y }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const v = (window as any).drapewright.ui.get().viewport
      const scale = v.pxPerMm * v.zoom
      const r = document.querySelector('canvas')!.getBoundingClientRect()
      return {
        x: r.left + (x - v.cx) * scale + v.width / 2,
        y: r.top + (y - v.cy) * scale + v.height / 2,
      }
    },
    { x, y },
  )
}

/**
 * A one-finger drag, as synthetic pointer events (fingers edit until a
 * Pencil is seen). Sent in one go: a finger still within its slop when the
 * long-press timer fires opens the context menu instead, and a busy page
 * (the development build, a loaded machine) can delay a timer-paced move
 * that long.
 */
export async function fingerDrag(
  page: Page,
  from: { x: number; y: number },
  to: { x: number; y: number },
) {
  await page.evaluate(
    ({ from, to }) => {
      const canvas = document.querySelector('canvas')!
      const send = (type: string, at: { x: number; y: number }) =>
        canvas.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            pointerId: 200,
            pointerType: 'touch',
            isPrimary: true,
            clientX: at.x,
            clientY: at.y,
          }),
        )
      send('pointerdown', from)
      for (let k = 1; k <= 8; k++) {
        send('pointermove', {
          x: from.x + ((to.x - from.x) * k) / 8,
          y: from.y + ((to.y - from.y) * k) / 8,
        })
      }
      send('pointerup', to)
    },
    { from, to },
  )
}

export async function placementOf(page: Page, index: number) {
  return page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (i) => (window as any).drapewright.engine.document().pieces[i].placement,
    index,
  )
}

/** Sends a quick tap with `fingers` touches at once, as synthetic pointer events. */
export async function multiTap(page: Page, at: { x: number; y: number }, fingers: number) {
  await page.evaluate(
    async ({ at, fingers }) => {
      const canvas = document.querySelector('canvas')!
      const send = (type: string, id: number, dx: number) =>
        canvas.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            pointerId: 100 + id,
            pointerType: 'touch',
            isPrimary: id === 0,
            clientX: at.x + dx,
            clientY: at.y,
          }),
        )
      for (let i = 0; i < fingers; i++) send('pointerdown', i, i * 50)
      await new Promise((r) => setTimeout(r, 60))
      for (let i = 0; i < fingers; i++) send('pointerup', i, i * 50)
    },
    { at, fingers },
  )
}

/** The shortcut modifier the app expects for this browser's platform. */
export async function modKey(page: Page): Promise<'Meta' | 'Control'> {
  const apple = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.userAgent))
  return apple ? 'Meta' : 'Control'
}

/** Page coordinates of the middle of an edge (on the stitching line). */
export async function edgeMiddle(page: Page, pieceIndex: number, edgeId: string) {
  return page.evaluate(
    ({ i, edgeId }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const s = (window as any).drapewright
      const v = s.ui.get().viewport
      const sp = s.scene()[i]
      const ev = sp.view.edges.find((e: { id: string }) => e.id === edgeId)
      const scale = v.pxPerMm * v.zoom
      const r = document.querySelector('canvas')!.getBoundingClientRect()
      return {
        x: r.left + (ev.middle[0] + sp.piece.placement.x - v.cx) * scale + v.width / 2,
        y: r.top + (ev.middle[1] + sp.piece.placement.y - v.cy) * scale + v.height / 2,
      }
    },
    { i: pieceIndex, edgeId },
  )
}

/** The piece's edges as `[id, kind, endCorner, allowance]`. */
export async function edgesOf(page: Page, pieceIndex: number) {
  return page.evaluate(
    (i) =>
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).drapewright.engine
        .document()
        .pieces[i].outline.edges.map(
          (e: { id: string; kind: string; endCorner: string; allowance: number | null }) => [
            e.id,
            e.kind,
            e.endCorner,
            e.allowance,
          ],
        ),
    pieceIndex,
  )
}

/** Page coordinates of a work-table point (mm). */
export async function toPage(page: Page, x: number, y: number) {
  return page.evaluate(
    ({ x, y }) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const v = (window as any).drapewright.ui.get().viewport
      const scale = v.pxPerMm * v.zoom
      const r = document.querySelector('canvas')!.getBoundingClientRect()
      return {
        x: r.left + (x - v.cx) * scale + v.width / 2,
        y: r.top + (y - v.cy) * scale + v.height / 2,
      }
    },
    { x, y },
  )
}

/** The piece's notches as `[kind, at, from]`, and where the first mark meets the cutting line. */
export async function notchesOf(page: Page, pieceIndex: number) {
  return page.evaluate((i) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const piece = s.engine.document().pieces[i]
    const marks = s.scene()[i].view.notches
    return {
      notches: piece.notches.map((n: { kind: string; at: number; from: string }) => [
        n.kind,
        n.at,
        n.from,
      ]),
      firstCut: marks[0]?.onCut ?? null,
    }
  }, pieceIndex)
}

/** Whether a screenshot shows something in its middle unlike its corner. */
export async function drawnInTheMiddle(page: Page, png: Buffer): Promise<boolean> {
  return page.evaluate(async (data) => {
    const image = new Image()
    image.src = `data:image/png;base64,${data}`
    await image.decode()
    const canvas = new OffscreenCanvas(image.width, image.height)
    const context = canvas.getContext('2d')!
    context.drawImage(image, 0, 0)
    const at = (x: number, y: number) => context.getImageData(x, y, 1, 1).data
    const corner = at(4, 4)
    // Down the middle, where the figure stands.
    for (let y = Math.round(image.height * 0.3); y < image.height * 0.7; y += 4) {
      const p = at(Math.round(image.width / 2), y)
      const difference = [0, 1, 2].reduce((d, i) => d + Math.abs(p[i]! - corner[i]!), 0)
      if (difference > 30) return true
    }
    return false
  }, png.toString('base64'))
}

/** The share of two screenshots' pixels that differ, 0 to 1. */
export async function differing(page: Page, a: Buffer, b: Buffer): Promise<number> {
  return page.evaluate(
    async ([a, b]) => {
      const pixels = async (data: string) => {
        const image = new Image()
        image.src = `data:image/png;base64,${data}`
        await image.decode()
        const canvas = new OffscreenCanvas(image.width, image.height)
        const context = canvas.getContext('2d')!
        context.drawImage(image, 0, 0)
        return context.getImageData(0, 0, image.width, image.height).data
      }
      const [p, q] = [await pixels(a), await pixels(b)]
      if (p.length !== q.length) return 1
      let changed = 0
      for (let i = 0; i < p.length; i += 4) {
        const d =
          Math.abs(p[i]! - q[i]!) +
          Math.abs(p[i + 1]! - q[i + 1]!) +
          Math.abs(p[i + 2]! - q[i + 2]!)
        if (d > 30) changed++
      }
      return changed / (p.length / 4)
    },
    [a.toString('base64'), b.toString('base64')],
  )
}

/**
 * The 3D view's picture, the figure drawn, once it has held still for half
 * a second. The worker draws the figure's first frame at the size the view
 * had when asked; the pane's header then settles (the status that comes
 * with the figure wraps differently), and the frames after are drawn at the
 * view's new size. A picture of that first frame, stretched to the new
 * size, matches none after it.
 */
export async function steadyPicture(page: Page, view: Locator): Promise<Buffer> {
  let last: Buffer | null = null
  await expect
    .poll(
      async () => {
        const shot = await view.screenshot()
        const still =
          last !== null &&
          (await differing(page, last, shot)) < 0.002 &&
          (await drawnInTheMiddle(page, shot))
        last = shot
        return still
      },
      { intervals: [500], timeout: 10_000 },
    )
    .toBe(true)
  return last!
}
