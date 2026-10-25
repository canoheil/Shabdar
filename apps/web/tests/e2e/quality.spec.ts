import { readFileSync } from 'node:fs'
import { mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import {
  expectNoOverflow,
  expectNoSeriousA11y,
  expectTouchTargets,
} from './support/quality'
import { readFixtures } from './support/stack'

const ARTIFACTS = resolve(process.cwd(), 'quality-artifacts')
const VIEWPORTS = [
  { width: 375, height: 812 },
  { width: 768, height: 1024 },
  { width: 1024, height: 900 },
  { width: 1440, height: 900 },
] as const

async function applyStorage(page: Page, storagePath: string): Promise<void> {
  const state = JSON.parse(readFileSync(storagePath, 'utf8')) as {
    cookies: Parameters<BrowserContext['addCookies']>[0]
  }
  await page.context().addCookies(state.cookies)
}

function monitorRuntime(page: Page) {
  const consoleErrors: string[] = []
  const pageErrors: string[] = []
  const externalRequests: string[] = []
  const expectedOrigin = new URL(process.env.E2E_BASE_URL ?? 'http://127.0.0.1:3000').origin
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })
  page.on('pageerror', (error) => pageErrors.push(error.message))
  page.on('request', (request) => {
    if (!/^https?:/.test(request.url())) return
    if (new URL(request.url()).origin !== expectedOrigin) externalRequests.push(request.url())
  })
  return { consoleErrors, pageErrors, externalRequests }
}

async function selectTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
  await page.getByRole('combobox', { name: 'پوسته', exact: true }).selectOption(theme)
  if (theme === 'dark') await expect(page.locator('html')).toHaveClass(/(^|\s)dark(\s|$)/)
  else await expect(page.locator('html')).not.toHaveClass(/(^|\s)dark(\s|$)/)
}

async function waitForEnhancedFont(page: Page): Promise<void> {
  await page.waitForFunction(() => document.documentElement.classList.contains('fonts-enhanced'))
  await page.evaluate(() => document.fonts.ready.then(() => undefined))
}

async function auditAndCapture(page: Page, label: string, width: number, theme: 'light' | 'dark') {
  await expectNoSeriousA11y(page)
  await expectNoOverflow(page)
  await expectTouchTargets(page)
  await mkdir(ARTIFACTS, { recursive: true })
  await page.screenshot({
    path: resolve(ARTIFACTS, `${label}-${width}-${theme}.png`),
    fullPage: true,
    animations: 'disabled',
  })
}

const routes = [
  { label: 'home', path: () => '/', role: null },
  { label: 'detail', path: () => readFixtures().detailPath, role: null },
  { label: 'login', path: () => '/login', role: null },
  { label: 'post', path: () => '/post', role: 'seller' },
  { label: 'dashboard', path: () => '/me', role: 'seller' },
  { label: 'favorites', path: () => '/me/favorites', role: 'seller' },
  { label: 'admin', path: () => '/admin/queue', role: 'admin' },
] as const

for (const route of routes) {
  test(`@quality ${route.label} responsive light/dark matrix has no runtime or accessibility regressions`, async ({ page }) => {
    test.setTimeout(240_000)
    const runtime = monitorRuntime(page)
    await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'light'))
    if (route.role) await applyStorage(page, readFixtures()[`${route.role}Storage`])
    await page.goto(route.path())
    await expect(page.locator('main')).toBeVisible()
    await waitForEnhancedFont(page)

    for (const viewport of VIEWPORTS) {
      await page.setViewportSize(viewport)
      for (const theme of ['light', 'dark'] as const) {
        await selectTheme(page, theme)
        await auditAndCapture(page, route.label, viewport.width, theme)
      }
    }

    expect(runtime.externalRequests, 'No third-party runtime requests').toEqual([])
    expect(runtime.pageErrors, 'No uncaught page errors').toEqual([])
    expect(runtime.consoleErrors, 'No unexpected console errors').toEqual([])
  })
}

test('@quality phone landscape reduced motion and focused search stay clear of fixed navigation', async ({ page }) => {
  test.setTimeout(120_000)
  const runtime = monitorRuntime(page)
  await page.setViewportSize({ width: 844, height: 390 })
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'light'))
  await page.goto('/')
  await waitForEnhancedFont(page)

  for (const theme of ['light', 'dark'] as const) {
    await selectTheme(page, theme)
    await auditAndCapture(page, 'home-landscape-reduced', 844, theme)
  }

  const motion = await page.locator('.shell__brand').evaluate((element) => {
    const style = getComputedStyle(element)
    return {
      animation: Number.parseFloat(style.animationDuration),
      transition: Number.parseFloat(style.transitionDuration),
      scroll: style.scrollBehavior,
    }
  })
  expect(motion.animation).toBeLessThanOrEqual(0.001)
  expect(motion.transition).toBeLessThanOrEqual(0.001)
  expect(motion.scroll).toBe('auto')

  const search = page.getByRole('searchbox', { name: 'جستجو', exact: true })
  await search.focus()
  await expect(search).toBeFocused()
  const geometry = await search.evaluate((element) => {
    const rect = element.getBoundingClientRect()
    const navigation = document.querySelector('.shell__nav')
    if (!navigation) throw new Error('Main navigation is missing')
    const navigationRect = navigation.getBoundingClientRect()
    return {
      top: rect.top,
      bottom: rect.bottom,
      navigationTop: navigationRect.top,
      navigationPosition: getComputedStyle(navigation).position,
    }
  })
  expect(geometry.top).toBeGreaterThanOrEqual(0)
  if (geometry.navigationPosition === 'fixed') {
    expect(geometry.bottom).toBeLessThanOrEqual(geometry.navigationTop + 1)
  }

  expect(runtime.externalRequests).toEqual([])
  expect(runtime.pageErrors).toEqual([])
  expect(runtime.consoleErrors).toEqual([])
})

test('@quality 404 remains readable in both themes and its Home recovery works', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'light'))
  const response = await page.goto('/quality-route-that-does-not-exist')
  expect(response?.status()).toBe(404)
  await expect(page.locator('.error-page__status')).toHaveText('404')
  await waitForEnhancedFont(page)
  for (const theme of ['light', 'dark'] as const) {
    await selectTheme(page, theme)
    await auditAndCapture(page, 'not-found', 375, theme)
  }
  await page.getByRole('button', { name: 'بازگشت به خانه', exact: true }).click()
  await expect(page).toHaveURL(/\/$/)
  await expect(page.locator('#search')).toBeVisible()
})
