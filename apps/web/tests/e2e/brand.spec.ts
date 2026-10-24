import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, test } from '@playwright/test'
import { expectNoOverflow, expectNoSeriousA11y } from './support/quality'

function expectFlatLocalSvg(source: string): void {
  expect(source).not.toMatch(/<(?:image|filter|linearGradient|radialGradient)\b/i)
  expect(source).not.toMatch(/(?:href|src)\s*=\s*["'](?:https?:)?\/\//i)
  expect(source).not.toMatch(/(?:url\(\s*["']?https?:|data:image)/i)
}

test('@brand header mark is decorative local SVG and favicon is labelled', async ({ page, request }) => {
  const requests: string[] = []
  page.on('request', (browserRequest) => requests.push(browserRequest.url()))
  await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'light'))
  await page.goto('/')

  const brand = page.locator('.shell__brand')
  await expect(brand).toContainText('شبدر')
  const mark = brand.locator('svg[data-pose="mark"]')
  await expect(mark).toBeVisible()
  await expect(mark).toHaveAttribute('aria-hidden', 'true')
  await expect(mark).toHaveAttribute('viewBox', '0 0 96 96')
  expect(await mark.getAttribute('role')).toBeNull()
  const box = await mark.boundingBox()
  expect(box?.width).toBeGreaterThanOrEqual(32)
  expect(box?.height).toBeGreaterThanOrEqual(32)
  expectFlatLocalSvg(await mark.evaluate((element) => element.outerHTML))

  const icon = page.locator('link[rel~="icon"]')
  await expect(icon).toHaveAttribute('href', '/favicon.svg')
  await expect(icon).toHaveAttribute('type', 'image/svg+xml')
  const response = await request.get('/favicon.svg')
  expect(response.ok()).toBe(true)
  expect(response.headers()['content-type']).toContain('image/svg+xml')
  const favicon = await response.text()
  expectFlatLocalSvg(favicon)
  expect(favicon).toContain('<title id="shabdarak-title">شبدرُک</title>')
  expect(favicon).toContain('role="img"')
  expect(favicon).toContain('aria-labelledby="shabdarak-title"')

  expect(requests.some((url) => url.includes('shabdarak.png'))).toBe(false)
  const external = requests.filter((url) => (
    /^https?:/.test(url) && new URL(url).origin !== new URL(page.url()).origin
  ))
  expect(external).toEqual([])
  await expectNoOverflow(page)
})

test('@brand mark uses semantic colors at 375px in both themes', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'light'))
  await page.goto('/')
  const body = page.locator('.shell__brand .brand-mascot__body')
  await expect(body).toHaveCSS('fill', 'rgb(4, 120, 87)')
  await expectNoSeriousA11y(page)
  await expectNoOverflow(page)

  await page.getByRole('combobox', { name: 'پوسته', exact: true }).selectOption('dark')
  await expect(page.locator('html')).toHaveClass(/(^|\s)dark(\s|$)/)
  await expect(body).toHaveCSS('fill', 'rgb(110, 231, 183)')
  await expectNoSeriousA11y(page)
  await expectNoOverflow(page)
})

test('@brand component and favicon sources contain only flat local geometry', async () => {
  const component = readFileSync(resolve(process.cwd(), 'app/components/BrandMascot.vue'), 'utf8')
  const favicon = readFileSync(resolve(process.cwd(), 'public/favicon.svg'), 'utf8')
  expectFlatLocalSvg(component)
  expectFlatLocalSvg(favicon)
  expect(component).toContain("pose?: 'mark' | 'full' | 'search'")
  expect(component).toContain('viewBox="0 0 96 96"')
})
