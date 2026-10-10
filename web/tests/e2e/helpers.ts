import { fileURLToPath } from 'node:url'
import type { Page } from '@playwright/test'

export const fixture = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/svg/${name}`, import.meta.url))

/** Opens the app with the debug handle and imports a fixture. */
export async function openWithFixture(page: Page, name: string, add: string): Promise<void> {
  await page.goto('/?debug')
  await page.getByTestId('pattern-file').setInputFiles(fixture(name))
  await page.getByRole('dialog').getByRole('button', { name: add }).click()
}

/** Page coordinates of the middle of a piece, by index. */
export async function pieceCentre(page: Page, index: number): Promise<{ x: number; y: number }> {
  return page.evaluate((i) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).soom
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

export async function placementOf(page: Page, index: number) {
  return page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (i) => (window as any).soom.engine.document().pieces[i].placement,
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
      const s = (window as any).soom
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
      (window as any).soom.engine
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
      const v = (window as any).soom.ui.get().viewport
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
    const s = (window as any).soom
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
