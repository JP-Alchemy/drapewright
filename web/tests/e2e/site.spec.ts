import { expect, test } from './fixtures.ts'

test('the front page says what Drapewright is and opens the app', async ({ page, baseURL }) => {
  const origin = new URL(baseURL!).origin
  const foreign: string[] = []
  page.on('request', (req) => {
    if (new URL(req.url()).origin !== origin) foreign.push(req.url())
  })
  await page.goto('/')
  await expect(
    page.getByRole('heading', {
      level: 1,
      name: 'See how your sewing pattern fits before you cut',
    }),
  ).toBeVisible()
  // Found by search engines: a description, and one address for the page.
  await expect(page.locator('meta[name="description"]')).toHaveAttribute('content', /.{80,}/)
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
    'href',
    'https://drapewright.com/',
  )
  // Every link within the page goes somewhere on it.
  for (const href of await page
    .locator('a[href^="#"]')
    .evaluateAll((links) => links.map((a) => a.getAttribute('href')!))) {
    await expect(page.locator(href)).toHaveCount(1)
  }
  // The pictures are there.
  for (const img of await page.locator('main img').all()) {
    await img.scrollIntoViewIfNeeded()
    await expect
      .poll(() => img.evaluate((i: HTMLImageElement) => i.naturalWidth))
      .toBeGreaterThan(0)
  }
  expect(foreign).toEqual([])

  await page.getByRole('link', { name: 'Open Drapewright: free, no sign-up' }).click()
  await expect(page).toHaveURL(/\/app\/$/)
  await expect(page.getByText('Open a sewing pattern to lay it out here.')).toBeVisible()
})

test('the app, installed on a home screen, opens at /app/', async ({ page }) => {
  await page.goto('/app/')
  const href = await page.locator('link[rel="manifest"]').getAttribute('href')
  const manifest = await (await page.request.get(new URL(href!, page.url()).href)).json()
  expect(manifest.start_url).toBe('/app/')
  // The same app as when it was at the root.
  expect(manifest.id).toBe('/')
})

test('links to the site and the app show a picture when shared', async ({ page }) => {
  for (const path of ['/', '/app/']) {
    await page.goto(path)
    const meta = (property: string) =>
      page.locator(`meta[property="${property}"]`).getAttribute('content')
    const image = new URL((await meta('og:image'))!)
    expect(image.href).toBe('https://drapewright.com/og.jpg')
    expect(await meta('og:title')).toBeTruthy()
    expect(await meta('og:description')).toBeTruthy()
    await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute(
      'content',
      'summary_large_image',
    )
    // The picture is served here, at the size the page gives for it.
    const size = await page.evaluate(async (src) => {
      const img = new Image()
      img.src = src
      await img.decode()
      return [img.naturalWidth, img.naturalHeight]
    }, image.pathname)
    expect(size).toEqual([
      Number(await meta('og:image:width')),
      Number(await meta('og:image:height')),
    ])
  }
})
