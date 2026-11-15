import { readFileSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { expectNoOverflow, expectTouchTargets } from './support/quality'
import { readFixtures } from './support/stack'

const nav = '.shell__nav'
const actions = ['خانه', 'جستجو', 'ثبت', 'پیام‌ها', 'ذخیره‌ها', 'ورود']
// Every icon is a <use> of one hashed, same-origin sprite: root-relative, never a data: URL.
const SPRITE = /^\/_nuxt\/sprite\.[\w-]+\.svg#([a-z0-9-]+)$/

test('public pages return SSR navigation icons even without JavaScript', async ({ browser, request, baseURL }) => {
  const context = await browser.newContext({ baseURL, javaScriptEnabled: false })
  const page = await context.newPage()
  try {
    for (const path of ['/', '/privacy', readFixtures().detailPath]) {
      const response = await request.get(path)
      expect(response.status(), path).toBe(200)
      const html = await response.text()
      expect(html).toContain('class="shell__nav"')
      const browserResponse = await page.goto(path)
      expect(browserResponse?.status(), path).toBe(200)
      const navigation = page.locator(nav)
      const hrefs: string[] = []
      for (const label of actions) {
        const action = navigation.locator('.shell__nav-action').filter({ hasText: label })
        const found = await action.locator('svg[aria-hidden="true"] use').evaluateAll((nodes) => nodes.map((node) => node.getAttribute('href') ?? ''))
        expect(found.length, label).toBeGreaterThan(0)
        await expect(action.locator('svg:not([aria-hidden="true"])')).toHaveCount(0)
        // The shown icon really paints: an external <use> has a box only once the sprite loaded and its id resolved.
        const shown = action.locator('svg:visible use')
        await expect(shown, label).toHaveCount(1)
        await expect.poll(() => shown.evaluate((node) => (node as SVGGraphicsElement).getBBox().width), { message: label }).toBeGreaterThan(0)
        hrefs.push(...found)
      }
      for (const href of hrefs) expect(href).toMatch(SPRITE)
      const sprite = await request.get(hrefs[0]!.split('#')[0]!)
      expect(sprite.status()).toBe(200)
      expect(sprite.headers()['content-type']).toContain('image/svg+xml')
      const body = await sprite.text()
      for (const href of hrefs) expect(body).toContain(`<symbol id="${href.split('#')[1]}" `)
    }
  } finally {
    await context.close()
  }
})

test('shell hydrates without mismatch for anonymous and admin navigation', async ({ browser, baseURL }) => {
  const admin = JSON.parse(readFileSync(readFixtures().adminStorage, 'utf8'))
  for (const storageState of [undefined, admin]) {
    const context = await browser.newContext({ baseURL, storageState })
    const page = await context.newPage()
    const errors: string[] = []
    page.on('console', message => {
      if (/hydration|hydrating/i.test(message.text())) errors.push(message.text())
    })
    page.on('pageerror', error => errors.push(error.message))
    try {
      const response = await page.goto('/')
      expect(response?.status()).toBe(200)
      await expect(page.locator(`${nav} .shell__nav-action svg:visible`).first()).toBeVisible()
      if (storageState) {
        // Admin entries are desktop-only: at this 375px viewport they are rendered but display: none.
        await expect(page.getByRole('link', { name: 'صف بررسی', exact: true, includeHidden: true })).toHaveCount(1)
        await expect(page.locator('.shell__nav-item--admin svg use')).toHaveCount(6)
      }
      await page.getByRole('navigation', { name: 'ناوبری اصلی' }).getByRole('button', { name: 'جستجو', exact: true }).click()
      await expect(page.getByRole('searchbox', { name: 'جستجو', exact: true })).toBeFocused()
      expect(errors).toEqual([])
      // Hydration left every icon reference as the server wrote it.
      const hrefs = await page.locator(`${nav} svg use`).evaluateAll((nodes) => nodes.map((node) => node.getAttribute('href') ?? ''))
      expect(hrefs.length).toBeGreaterThan(0)
      for (const href of hrefs) expect(href).toMatch(SPRITE)
    } finally {
      await context.close()
    }
  }
})

test('the shell keeps its labels, names and sizes when the sprite cannot load', async ({ page }) => {
  await page.route(/\/_nuxt\/sprite\.[\w-]+\.svg$/, (route) => route.abort())
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto('/')
  const navigation = page.getByRole('navigation', { name: 'ناوبری اصلی' })
  for (const label of actions) await expect(navigation.getByText(label, { exact: true })).toBeVisible()
  await expect(page.getByRole('combobox', { name: 'پوسته', exact: true })).toBeVisible()
  await expect(page.getByRole('searchbox', { name: 'جستجو', exact: true })).toBeVisible()
  await expectTouchTargets(page)
  await expectNoOverflow(page)
})
