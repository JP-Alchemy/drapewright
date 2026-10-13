import { expect, test } from './fixtures.ts'

/** The avatar pane's place and size on the page. */
const boxOf = (page: import('@playwright/test').Page) =>
  page.getByRole('region', { name: 'Avatar' }).evaluate((el) => {
    const r = el.getBoundingClientRect()
    return {
      x: Math.round(r.x),
      y: Math.round(r.y),
      w: Math.round(r.width),
      h: Math.round(r.height),
    }
  })

test('the avatar beside the table is resized by its edge', async ({ page }) => {
  await page.goto('/app/')
  await page.getByRole('button', { name: 'Avatar' }).click()
  const edge = page.getByRole('separator', { name: 'Resize the avatar' })
  await expect(edge).toHaveAttribute('aria-valuenow', '50')
  const before = await boxOf(page)
  // Towards the table: the avatar gets more room.
  const beside = (await edge.getAttribute('aria-orientation')) === 'vertical'
  await edge.focus()
  await page.keyboard.press(beside ? 'ArrowLeft' : 'ArrowUp')
  await page.keyboard.press(beside ? 'ArrowLeft' : 'ArrowUp')
  await expect(edge).toHaveAttribute('aria-valuenow', '60')
  const after = await boxOf(page)
  expect(beside ? after.w : after.h).toBeGreaterThan(beside ? before.w : before.h)
  // Kept after a reload.
  await page.reload()
  await page.getByRole('button', { name: 'Avatar' }).click()
  await expect(page.getByRole('separator', { name: 'Resize the avatar' })).toHaveAttribute(
    'aria-valuenow',
    '60',
  )
})

test('the avatar floats over the table, moves and resizes, and goes back', async ({ page }) => {
  await page.goto('/app/')
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  await pane.getByRole('button', { name: /^Float over the table/ }).click()
  // The table has the room the avatar had.
  await expect(page.getByRole('separator', { name: 'Resize the avatar' })).toHaveCount(0)
  const table = page.getByRole('main', { name: 'Cutting table' })
  const tableBox = (await table.boundingBox())!
  const start = await boxOf(page)
  expect(start.x).toBeGreaterThanOrEqual(Math.round(tableBox.x))
  expect(start.x + start.w).toBeLessThanOrEqual(Math.round(tableBox.x + tableBox.width) + 1)

  // Sized by its corner, moved by its grip (keys here; a finger or mouse
  // drags them, or the title bar). It keeps within the room it has.
  await pane.getByRole('button', { name: 'Resize the avatar window' }).focus()
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowUp')
  await expect.poll(() => boxOf(page)).toMatchObject({ w: start.w - 20, h: start.h - 20 })
  await pane.getByRole('button', { name: 'Move the avatar window' }).focus()
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowDown')
  await expect.poll(() => boxOf(page)).toMatchObject({ x: start.x - 20, y: start.y + 20 })
  const moved = await boxOf(page)

  // Where it was left, after a reload.
  await page.reload()
  await page.getByRole('button', { name: 'Avatar' }).click()
  await expect.poll(() => boxOf(page)).toEqual(moved)

  // Back beside (or under) the table.
  await pane.getByRole('button', { name: /^Put back/ }).click()
  await expect(page.getByRole('separator', { name: 'Resize the avatar' })).toBeVisible()
})

test('the floating avatar is dragged by its title bar and its corner', async ({ page }, info) => {
  test.skip(info.project.name === 'ipad-webkit', 'mouse drag')
  await page.goto('/app/')
  await page.getByRole('button', { name: 'Avatar' }).click()
  const pane = page.getByRole('region', { name: 'Avatar' })
  await pane.getByRole('button', { name: /^Float over the table/ }).click()
  const start = await boxOf(page)
  // By the title bar, beside the heading.
  const title = pane.getByText('Avatar', { exact: true }).first()
  const t = (await title.boundingBox())!
  await page.mouse.move(t.x + t.width + 30, t.y + t.height / 2)
  await page.mouse.down()
  await page.mouse.move(t.x + t.width - 30, t.y + t.height / 2 + 10, { steps: 5 })
  await page.mouse.up()
  await expect.poll(() => boxOf(page)).toMatchObject({ x: start.x - 60, w: start.w })
  // By the corner: smaller.
  const c = (await pane.getByRole('button', { name: 'Resize the avatar window' }).boundingBox())!
  await page.mouse.move(c.x + c.width / 2, c.y + c.height / 2)
  await page.mouse.down()
  await page.mouse.move(c.x + c.width / 2 - 40, c.y + c.height / 2 - 50, { steps: 5 })
  await page.mouse.up()
  await expect.poll(() => boxOf(page)).toMatchObject({ w: start.w - 40 })
})
