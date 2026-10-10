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
