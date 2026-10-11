import { expect, test } from '@playwright/test'
import { fixture } from './helpers.ts'

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
  await page.getByRole('button', { name: 'Lengthen or shorten' }).click()
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
  await page.getByRole('button', { name: 'Bust adjustment' }).click()
  await expect(page.getByText('Tap the bust point.')).toBeVisible()
  await page.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByText('Tap the bust point.')).toHaveCount(0)

  await page.getByRole('button', { name: 'Bust adjustment' }).click()
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
  // A small bust adjustment instead.
  await page.getByText('Small bust', { exact: true }).click()
  await expect(page.getByRole('switch', { name: 'Small bust adjustment, 2.0 cm' })).toBeChecked()
  await expect(front).toContainText(/2[2-5]\.\d cm × 5[5-9]\.\d cm/)
})
