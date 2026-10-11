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
