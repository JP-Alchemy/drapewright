import { expect, type Page, test } from './fixtures.ts'

/** Jo's measurements, added by the debug handle (the Measurements dialog does the same). */
async function addJo(page: Page) {
  await page.goto('/app/?debug')
  await expect(page.getByText('Open a sewing pattern to lay it out here.')).toBeVisible()
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    s.dispatch({
      type: 'addMeasurementSet',
      set: {
        id: '',
        name: 'Jo',
        figure: 'woman',
        revision: 1,
        takenOn: '2026-10-14',
        age: null,
        values: {
          height: 1650,
          bust: 920,
          highBust: 860,
          waist: 740,
          highHip: 900,
          hip: 1000,
          neck: 340,
        },
        previous: [],
      },
    })
  })
}

test('a FreeSewing skirt is drafted to someone’s measurements and added', async ({ page }) => {
  await addJo(page)
  await page.getByRole('button', { name: /^A FreeSewing design, made to measure/ }).click()
  const dialog = page.getByRole('dialog', { name: 'Draft a FreeSewing design' })
  await dialog.getByText('Penelope pencil skirt', { exact: true }).click()
  await expect(dialog.getByRole('status')).toHaveText(
    /^Drafted to Jo’s .*waist.*\. Estimated from FreeSewing’s standard sizes, in proportion: .+\./,
  )
  await dialog.getByRole('button', { name: 'Draft Penelope pencil skirt' }).click()
  const found = page.getByRole('dialog')
  await expect(found.getByRole('heading', { name: 'Found 3 pieces' })).toBeVisible({
    timeout: 30_000,
  })
  await expect(found.getByText('Penelope pencil skirt for Jo.svg')).toBeVisible()
  await found.getByRole('button', { name: /^Add 3 pieces/ }).click()
  const list = page.getByRole('listbox', { name: 'Pieces' })
  await expect(list.getByRole('option')).toHaveCount(3)
  await expect(list.getByRole('option', { name: /^Front/ })).toContainText('on the fold')
})

test('every FreeSewing design offered drafts and opens', async ({ page }) => {
  test.setTimeout(180_000)
  await addJo(page)
  const designs = [
    'Teagan T-shirt',
    'Sasha dress',
    'Penelope pencil skirt',
    'Sandy circle skirt',
    'Cathrin corset',
    'Bella bodice block',
    'Simone shirt',
    'Diana draped top',
  ]
  for (const design of designs) {
    await page.getByRole('button', { name: /^A FreeSewing design, made to measure/ }).click()
    const dialog = page.getByRole('dialog', { name: 'Draft a FreeSewing design' })
    await dialog.getByText(design, { exact: true }).click()
    await expect(dialog.getByRole('status')).toHaveText(/^Drafted to Jo’s/)
    await dialog.getByRole('button', { name: `Draft ${design}` }).click()
    const found = page.getByRole('dialog')
    await expect(found.getByRole('heading', { name: /^Found \d+ piece/ }), design).toBeVisible({
      timeout: 30_000,
    })
    await found.getByRole('button', { name: 'Cancel' }).click()
  }
})

test('a drafted T-shirt is sewn by Find seams, its sleeves set into its armholes', async ({
  page,
}) => {
  await addJo(page)
  await page.getByRole('button', { name: /^A FreeSewing design, made to measure/ }).click()
  const dialog = page.getByRole('dialog', { name: 'Draft a FreeSewing design' })
  await dialog.getByText('Teagan T-shirt', { exact: true }).click()
  await expect(dialog.getByRole('status')).toHaveText(/^Drafted to Jo’s/)
  await dialog.getByRole('button', { name: 'Draft Teagan T-shirt' }).click()
  await page.getByRole('button', { name: 'Add 3 pieces' }).click()
  const seams = page.getByRole('region', { name: 'Seams' })
  await seams.getByRole('button', { name: 'Find seams' }).click()
  const found = seams.getByRole('group', { name: 'Edges that look sewn together' })
  // Shoulders, sides, the sleeves' underarms, and each sleeve's head round
  // its armhole: the front's half and the back's.
  await found.getByRole('button', { name: 'Sew all 5' }).click()
  await expect(seams.getByRole('button', { name: /^Seam \d/ })).toHaveCount(5)
  await expect(
    seams.getByRole('button', { name: /Sleeve · part of edge \d ↔ Front · edge \d/ }),
  ).toHaveCount(1)
  await expect(
    seams.getByRole('button', { name: /Sleeve · part of edge \d ↔ Back · edge \d/ }),
  ).toHaveCount(1)
})
