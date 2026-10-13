import { expect, type Page, test } from './fixtures.ts'
import { fixture } from './helpers.ts'

/** Opens the avatar; whether this browser draws it in 3D (not WebKit on Linux, as in CI). */
async function openTheAvatar(page: Page): Promise<boolean> {
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drawnWith = pane.getByText(/^Drawn with (WebGPU|WebGL2)$/)
  const cannot = pane.getByText('This browser can’t draw the avatar in 3D.')
  await expect(drawnWith.or(cannot)).toBeVisible({ timeout: 60_000 })
  return drawnWith.isVisible()
}

test('a draped skirt says how it fits, and is altered at its side seams to fit', async ({
  page,
}) => {
  test.setTimeout(300_000)
  await page.goto('/app/')
  await page.getByTestId('pattern-file').setInputFiles(fixture('skirt.drapewright'))
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(2)
  test.skip(!(await openTheAvatar(page)), 'Draping is seen in the 3D view')
  const pane = page.getByRole('region', { name: 'Avatar' })
  const drape = pane.getByRole('group', { name: 'Drape the garment' })
  await expect(pane.getByRole('status').first()).toHaveText(/standard figure/, { timeout: 60_000 })
  await drape.getByRole('button', { name: 'Drape', exact: true }).click()
  const fit = drape.getByRole('group', { name: 'Fit' })
  // What it says of the fit (not the drape's status, over the view).
  const said = fit.getByText(/^(For |It fits)/)
  // Fitted, it should have about 6 cm of ease at the hip: it has hardly any.
  await expect(said).toHaveText(
    /^For fitted: \d+\.\d cm too tight at the hip; right at the waist\.$/,
    { timeout: 180_000 },
  )
  await fit.getByRole('button', { name: 'Alter to fit' }).click()
  await expect(
    page.getByText(
      /^Let out \d+\.\d cm at the hip, at the side seams of “Skirt front” and “Skirt back”/,
    ),
  ).toBeVisible()
  // Each piece has its step, and the skirt, draped again, fits.
  const front = page.getByRole('listbox', { name: 'Pieces' }).getByRole('option').first()
  await expect(front).toBeVisible()
  await expect(said).toHaveText('It fits as fitted should at the waist and hip.', {
    timeout: 180_000,
  })
  // Close-fitting, the same skirt is too loose.
  await fit.getByRole('button', { name: /Fitted/ }).click()
  await page.getByRole('option', { name: 'Close (corsets, stretch)' }).click()
  await expect(said).toHaveText(/too loose at the waist/)
})
