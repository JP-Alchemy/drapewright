import { readFile } from 'node:fs/promises'
import { expect, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

test('feedback starts an email saying which version and browser', async ({ page }) => {
  await page.goto('/app/')
  await page.getByRole('button', { name: 'Feedback' }).click()
  const dialog = page.getByRole('dialog', { name: 'Feedback' })
  const email = dialog.getByRole('link', { name: 'Email feedback' })
  const href = decodeURIComponent((await email.getAttribute('href'))!)
  expect(href).toMatch(/^mailto:juan\.bothma@gmail\.com\?subject=Drapewright feedback&body=/)
  expect(href).toMatch(/Drapewright \w+, engine \d+\.\d+\.\d+/)
  expect(href).toContain('Browser: ')
  await expect(dialog.getByRole('link', { name: 'Connect on LinkedIn' })).toHaveAttribute(
    'href',
    'https://www.linkedin.com/in/jp-bothma/',
  )
})

test('a pattern that didn’t open right is reported, with what was read', async ({ page }) => {
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('aama-skirt.dxf'))
  const dialog = page.getByRole('dialog')
  await dialog.getByRole('button', { name: 'Didn’t open right?' }).click()
  await dialog.getByRole('textbox', { name: 'What’s wrong' }).fill('The waistband is missing.')
  const href = decodeURIComponent(
    (await dialog.getByRole('link', { name: 'Email the report' }).getAttribute('href'))!,
  )
  expect(href).toContain('subject=Drapewright: “aama-skirt.dxf” didn’t open right')
  expect(href).toContain('The waistband is missing.')
  expect(href).toMatch(/File: aama-skirt\.dxf \(\d+ KB, [0-9a-f]{16}…\)/)
  expect(href).toMatch(/Pieces found: \d+/)
  // Saved, to attach.
  const saved = page.waitForEvent('download')
  await dialog.getByRole('button', { name: 'Save the report' }).click()
  const report = await saved
  expect(report.suggestedFilename()).toBe('aama-skirt.dxf report.txt')
  const text = await readFile((await report.path())!, 'utf8')
  expect(text).toContain('The waistband is missing.')
  expect(text).toMatch(/Pieces found: \d+/)
})
