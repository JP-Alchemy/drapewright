import { expect, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

test('the pieces are laid out on fabric, and say how much it takes', async ({ page }) => {
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('blouse.drapewright'))
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(3)
  await page.getByRole('button', { name: 'Layout' }).click()
  const dialog = page.getByRole('dialog', { name: 'Cutting layout' })
  const takes = dialog.getByRole('status')
  // Folded 150 cm fabric, to start.
  await expect(takes).toHaveText(
    /^It takes \d\.\d\d m of main fabric 150 cm wide, folded, \d+% of it cut into pieces\./,
  )
  const folded = Number(/(\d\.\d\d) m/.exec((await takes.textContent()) ?? '')![1])
  await expect(dialog.getByRole('img', { name: /laid out on the fabric/ })).toBeVisible()
  // Open, the same pieces lie side by side: no more fabric than folded.
  await dialog.getByText('Folded in half lengthwise').click()
  await expect(takes).not.toHaveText(/folded/)
  const open = Number(/(\d\.\d\d) m/.exec((await takes.textContent()) ?? '')![1])
  expect(open).toBeLessThanOrEqual(folded + 0.05)
  // A narrower fabric takes more.
  await dialog.getByText('Folded in half lengthwise').click()
  await dialog.getByRole('button', { name: /Fabric width/ }).click()
  await page.getByRole('option', { name: '112 cm' }).click()
  await expect(takes).toHaveText(/of main fabric 112 cm wide, folded/)
  const narrow = Number(/(\d\.\d\d) m/.exec((await takes.textContent()) ?? '')![1])
  expect(narrow).toBeGreaterThanOrEqual(folded)
  // Kept for next time.
  await dialog.getByRole('button', { name: 'Close' }).click()
  await page.getByRole('button', { name: 'Layout' }).click()
  await expect(dialog.getByRole('status')).toHaveText(/112 cm wide, folded/)
})
