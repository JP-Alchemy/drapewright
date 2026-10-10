import { readFile } from 'node:fs/promises'
import { expect, test } from '@playwright/test'
import { openWithFixture } from './helpers.ts'

test('saves a tiled PDF that prints at true size', async ({ page }) => {
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  await page.getByRole('button', { name: 'Print', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Print at true size' })
  const summary = dialog.getByTestId('print-summary')
  await expect(summary).toContainText('of A4')
  await expect(dialog.getByRole('img', { name: /Layout of the/ })).toBeVisible()

  // US Letter, as most printers in North America take.
  await dialog.getByRole('button', { name: /Paper/ }).click()
  await page.getByRole('option', { name: 'US Letter' }).click()
  await expect(summary).toContainText('of US Letter')
  const pages = Number((await summary.textContent())?.match(/(\d+) pages?/)?.[1])
  expect(pages).toBeGreaterThan(1)

  const download = page.waitForEvent('download')
  await dialog.getByRole('button', { name: 'Save PDF' }).click()
  const file = await download
  expect(file.suggestedFilename()).toBe('Untitled pattern.pdf')
  const pdf = (await readFile(await file.path())).toString('latin1')
  expect(pdf.startsWith('%PDF-')).toBe(true)
  // Readers are told to print at 100 %, and every page is Letter size.
  expect(pdf).toContain('/PrintScaling /None')
  expect(pdf.match(/\/Type \/Page\b/g)).toHaveLength(pages)
  expect(pdf.match(/\/MediaBox \[0 0 612 792\]/g)).toHaveLength(pages)
  await expect(dialog).toBeHidden()

  // The choice is part of the project, and can be undone.
  await page.getByRole('button', { name: 'Undo print settings' }).click()
  await page.getByRole('button', { name: 'Print', exact: true }).click()
  await expect(page.getByTestId('print-summary')).toContainText('of A4')
})

test('measuring the screen makes 100% true size, and is remembered', async ({ page }) => {
  await page.goto('/?debug')
  await page.getByRole('button', { name: /Measure the screen \(not measured/ }).click()
  const dialog = page.getByRole('dialog', { name: 'Measure your screen' })
  const card = dialog.getByTestId('card-outline')
  // Until measured, the web's nominal 96 px to the inch.
  const before = (await card.boundingBox())!.width
  expect(before).toBeCloseTo((85.6 * 96) / 25.4, 0)
  const larger = dialog.getByRole('button', { name: 'A little larger' })
  for (let i = 0; i < 4; i++) await larger.click()
  const after = (await card.boundingBox())!.width
  expect(after / before).toBeCloseTo(1.0025 ** 4, 3)
  await dialog.getByRole('button', { name: 'Save' }).click()
  await expect(dialog).toBeHidden()

  const pxPerMm = () =>
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    page.evaluate(() => (window as any).soom.ui.get().viewport.pxPerMm as number)
  const measured = await pxPerMm()
  expect(measured * 85.6).toBeCloseTo(after, 0)
  await page.reload()
  await page.waitForFunction(() => 'soom' in window)
  expect(await pxPerMm()).toBeCloseTo(measured, 6)
  await expect(page.getByRole('button', { name: 'Measure the screen', exact: true })).toBeVisible()
})
