import { expect, test } from './fixtures.ts'

test('a sample blouse opens sewn, as a project of its own, beside the figure', async ({ page }) => {
  await page.goto('/app/')
  const samples = page.getByRole('group', { name: 'No pattern to hand? Try one of ours:' })
  await samples.getByRole('button', { name: /^A blouse, ready to drape/ }).click()
  await expect(
    page.getByRole('button', { name: /^Projects \(open now: Sample blouse\)/ }),
  ).toBeVisible()
  const pieces = page.getByRole('treegrid', { name: 'Pieces' })
  await expect(pieces.getByRole('row', { level: 1 })).toHaveCount(3)
  // Sewn: a piece opens to show its seams.
  await pieces
    .getByRole('button', { name: /^Expand/ })
    .first()
    .click()
  await expect(pieces.getByRole('row', { level: 2 }).first()).toBeVisible()
  await expect(page.getByRole('region', { name: 'Avatar' })).toBeVisible()
  await expect(page.getByText(/is open as a project of its own: press Drape/)).toBeVisible()
})

test('a sample asked for in the address opens once', async ({ page }) => {
  await page.goto('/app/?sample=skirt')
  await expect(
    page.getByRole('button', { name: /^Projects \(open now: Sample skirt\)/ }),
  ).toBeVisible()
  await expect(
    page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', { level: 1 }),
  ).toHaveCount(2)
  // Taken out of the address, so a reload doesn't open another.
  await expect(page).toHaveURL(/\/app\/$/)
})

test('the sample dress is read from its PDF, and its licence is there', async ({ page }) => {
  await page.goto('/app/')
  const samples = page.getByRole('group', { name: 'No pattern to hand? Try one of ours:' })
  const licence = await samples.getByRole('link', { name: 'Licence' }).getAttribute('href')
  const text = await (await page.request.get(licence!)).text()
  expect(text).toContain('MIT License')
  expect(text).toContain('Joost De Cock')
  await samples.getByRole('button', { name: /^A dress from a PDF pattern/ }).click()
  const dialog = page.getByRole('dialog')
  // Drafted without its pockets: the side front whole.
  await expect(dialog.getByRole('heading', { name: 'Found 5 pieces' })).toBeVisible()
})
