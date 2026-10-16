import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { expect, test } from './fixtures.ts'
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
    () => (window as any).drapewright.io.call('storageStatus') as Promise<{ kind: string }>,
  )
  if (test.info().project.name === 'chromium') expect(store.kind).toBe('opfs')
  else expect(['opfs', 'indexeddb']).toContain(store.kind)

  await page.reload()
  await expect(page.getByRole('button', { name: /open now: Skirt for Anna/ })).toBeVisible()
  const list = page.getByRole('treegrid', { name: 'Pieces' })
  await expect(list.getByRole('row', { level: 1 })).toHaveCount(2)
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
  await expect(
    page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', { level: 1 }),
  ).toHaveCount(1)

  // Back up: a .drapewright file with the project and the SVG it came from.
  const download = page.waitForEvent('download')
  await page.getByRole('button', { name: 'Back up' }).click()
  const file = await download
  // WebKit on Linux saves spaces in file names as underscores.
  expect(file.suggestedFilename()).toMatch(/^Untitled[ _]pattern\.drapewright$/)
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
  await expect(
    page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', { level: 1 }),
  ).toHaveCount(1)

  // Delete the original, after confirming.
  await page.getByRole('button', { name: projectsButton }).click()
  await dialog.getByRole('button', { name: 'Delete “Untitled pattern”' }).click()
  await expect(dialog.getByRole('alertdialog')).toContainText('This cannot be undone')
  await dialog.getByRole('button', { name: 'Delete', exact: true }).click()
  await expect(dialog.getByRole('button', { name: /Untitled pattern.*1 piece/ })).toHaveCount(0)
})

test('projects saved before the rename (as Soom) still open', async ({ page }) => {
  // A project stored the way the app did under its earlier name, before this
  // version ever opened here (as for someone updating): its list in the
  // "soom" database, its format called "soom-project".
  const json = JSON.parse(
    await readFile(
      fileURLToPath(
        new URL('../../../crates/project/tests/fixtures/project-v4.json', import.meta.url),
      ),
      'utf8',
    ),
  )
  json.format = 'soom-project'
  json.meta.id = 'earlier-1'
  json.meta.name = 'Saved before the rename'
  // A page of the site that is not the app, to store it from.
  await page.goto('/app/icon.svg')
  await page.evaluate(async (text) => {
    const bytes = new TextEncoder().encode(text)
    await new Promise<void>((resolve, reject) => {
      const open = indexedDB.open('soom', 2)
      open.onupgradeneeded = () => {
        open.result.createObjectStore('projects', { keyPath: 'id' })
        open.result.createObjectStore('files')
      }
      open.onerror = () => reject(open.error)
      open.onsuccess = () => {
        const db = open.result
        const tx = db.transaction(['projects', 'files'], 'readwrite')
        tx.objectStore('projects').put({
          id: 'earlier-1',
          name: 'Saved before the rename',
          modified: '2026-10-10T10:00:00Z',
          pieces: 2,
        })
        tx.objectStore('files').put(bytes, ['earlier-1', 'project.json'])
        tx.oncomplete = () => {
          db.close()
          resolve()
        }
        tx.onerror = () => reject(tx.error)
      }
    })
    // And in OPFS, where browsers that can keep project files there (not
    // WebKit here, which keeps them in the database).
    try {
      const root = await navigator.storage.getDirectory()
      const projects = await root.getDirectoryHandle('projects', { create: true })
      const dir = await projects.getDirectoryHandle('earlier-1', { create: true })
      const file = await dir.getFileHandle('project.json', { create: true })
      const writer = await file.createWritable()
      await writer.write(bytes)
      await writer.close()
    } catch {
      // No OPFS from here: the database has it.
    }
  }, JSON.stringify(json))
  await page.goto('/app/')
  await expect(
    page.getByRole('button', { name: /open now: Saved before the rename/ }),
  ).toBeVisible()
  await expect(
    page.getByRole('treegrid', { name: 'Pieces' }).getByRole('row', { level: 1 }),
  ).toHaveCount(2)
})
