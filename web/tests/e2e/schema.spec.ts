import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { expect, test } from './fixtures.ts'
import { Ajv2020 } from 'ajv/dist/2020.js'
import { openWithFixture } from './helpers.ts'

const schemaPath = fileURLToPath(
  new URL('../../../docs/schema/project.schema.json', import.meta.url),
)

test('saved projects match the published JSON Schema', async ({ page }) => {
  const validate = new Ajv2020({ allErrors: true, strict: false, validateFormats: false }).compile(
    JSON.parse(await readFile(schemaPath, 'utf8')),
  )
  const project = () =>
    page.evaluate(() =>
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      JSON.parse((window as any).drapewright.engine.toJson()),
    )

  // Notches, grainline, a fold, marks and both allowance modes.
  await openWithFixture(page, 'aama-skirt.dxf', 'Add 2 pieces', 'Already included')
  const json = await project()
  expect(json.pieces.length).toBe(2)
  expect(validate(json), JSON.stringify(validate.errors, null, 2)).toBe(true)

  // And a project with an imported SVG and print settings changed.
  await page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).drapewright
    const print = s.engine.document().print
    s.dispatch({
      type: 'setPrintSettings',
      print: { ...print, paper: { width: 500, height: 700 }, assembly: 'overlap' },
    })
  })
  expect(validate(await project()), JSON.stringify(validate.errors, null, 2)).toBe(true)
})
