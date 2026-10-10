import { readFile } from 'node:fs/promises'
import { expect, test } from '@playwright/test'
import { openWithFixture } from './helpers.ts'

const projectsButton = /Projects \(open now/

test('work is saved on the device and comes back after a reload', async ({ page }) => {
  await openWithFixture(page, 'aama-skirt.dxf', 'Add 2 pieces', 'Already included')
  await page.getByRole('button', { name: projectsButton }).click()
  const dialog = page.getByRole('dialog', { name: 'Projects' })
  const name = dialog.getByRole('textbox', { name: 'Project name' })
  await name.fill('Skirt for Anna')
  await name.press('Enter')
  await dialog.getByRole('button', { name: 'Close' }).click()
  await expect(page.getByTestId('save-status')).toHaveText('Saved')

  // Chromium keeps projects in OPFS. WebKit's test contexts are private, so
  // on some platforms it falls back to IndexedDB, as Safari does in private
  // browsing.
  const store = await page.evaluate(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    () => (window as any).soom.io.call('storageStatus') as Promise<{ kind: string }>,
  )
  if (test.info().project.name === 'chromium') expect(store.kind).toBe('opfs')
  else expect(['opfs', 'indexeddb']).toContain(store.kind)

  await page.reload()
  await expect(page.getByRole('button', { name: /open now: Skirt for Anna/ })).toBeVisible()
  const list = page.getByRole('listbox', { name: 'Pieces' })
  await expect(list.getByRole('option')).toHaveCount(2)
  await expect(page.getByTestId('save-status')).toHaveText('Saved')
})

test('projects can be switched, backed up, restored and deleted', async ({ page }) => {
  await openWithFixture(page, 'flared-skirt.svg', 'Add the piece')
  await expect(page.getByTestId('save-status')).toHaveText('Saved')

  // A second project, then back to the first.
  await page.getByRole('button', { name: projectsButton }).click()
  await page.getByRole('button', { name: 'New project' }).click()
  await expect(page.getByText('No pieces yet.')).toBeVisible()
  await page.getByRole('button', { name: projectsButton }).click()
  const dialog = page.getByRole('dialog', { name: 'Projects' })
  await dialog.getByRole('button', { name: /Untitled pattern.*1 piece/ }).click()
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(1)

  // Back up: a .soom file with the project and the SVG it came from.
  const download = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Back up' }).click()
  const file = await download
  // WebKit on Linux saves spaces in file names as underscores.
  expect(file.suggestedFilename()).toMatch(/^Untitled[ _]pattern\.soom$/)
  const path = await file.path()
  const zip = (await readFile(path)).toString('latin1')
  expect(zip.startsWith('PK')).toBe(true)
  expect(zip).toContain('project.json')
  expect(zip).toMatch(/sources\/[0-9a-f]{64}-flared-skirt\.svg/)

  // Restoring it beside the original makes a copy; nothing is overwritten.
  await page.getByRole('button', { name: projectsButton }).click()
  await page.getByTestId('restore-file').setInputFiles(path)
  await expect(
    page.getByRole('button', { name: /open now: Untitled pattern \(restored\)/ }),
  ).toBeVisible()
  await expect(page.getByRole('listbox', { name: 'Pieces' }).getByRole('option')).toHaveCount(1)

  // Delete the original, after confirming.
  await page.getByRole('button', { name: projectsButton }).click()
  await dialog.getByRole('button', { name: 'Delete “Untitled pattern”' }).click()
  await expect(dialog.getByRole('alertdialog')).toContainText('This cannot be undone')
  await dialog.getByRole('button', { name: 'Delete', exact: true }).click()
  await expect(dialog.getByRole('button', { name: /Untitled pattern.*1 piece/ })).toHaveCount(0)
})
