import { expect, test, type Page } from '@playwright/test'
import {
  expectNoOverflow,
  expectNoSeriousA11y,
  expectTouchTargets,
} from './support/quality'

const LIGHT = {
  '--color-bg': '#FBF7F0',
  '--color-surface': '#FFFFFF',
  '--color-surface-muted': '#F4ECDF',
  '--color-text': '#1F1B16',
  '--color-text-muted': '#675C4F',
  '--color-primary': '#047857',
  '--color-on-primary': '#FFFFFF',
  '--color-primary-hover': '#065F46',
  '--color-primary-soft': '#E2F1EA',
  '--color-border': '#EADFCF',
  '--color-control-border': '#8F8474',
  '--color-focus': '#047857',
  '--color-accent': '#A2470B',
  '--color-accent-soft': '#FCEFD6',
  '--color-accent-fill': '#F2B33D',
  '--color-on-accent-fill': '#3A2903',
  '--color-danger': '#B42318',
  '--color-on-danger': '#FFFFFF',
  '--color-error': '#B42318',
  '--color-error-bg': '#FDECE9',
  '--color-warning': '#8A5A00',
  '--color-warning-bg': '#FCEFD6',
  '--color-info': '#675C4F',
  '--color-info-bg': '#F4ECDF',
  '--color-tint-a': '#FCEFD6',
  '--color-ink-a': '#8A5A00',
  '--color-tint-b': '#E2F1EA',
  '--color-ink-b': '#047857',
  '--color-tint-c': '#FBE5DA',
  '--color-ink-c': '#A2470B',
  '--color-bubble': '#047857',
  '--color-on-bubble': '#FFFFFF',
  '--color-bubble-meta': '#D5F5E6',
  '--color-hero': '#0B3D2E',
  '--color-hero-deep': '#062A20',
  '--color-hero-text': '#F4FBF7',
  '--color-hero-muted': '#C4D9CE',
  '--color-hero-accent': '#F3C876',
  '--color-hero-border': '#396455',
} as const

const DARK = {
  '--color-bg': '#14120E',
  '--color-surface': '#1E1B16',
  '--color-surface-muted': '#28241D',
  '--color-text': '#F6F1E8',
  '--color-text-muted': '#BDB2A1',
  '--color-primary': '#6EE7B7',
  '--color-on-primary': '#062E22',
  '--color-primary-hover': '#A7F3D0',
  '--color-primary-soft': '#16342A',
  '--color-border': '#3A342A',
  '--color-control-border': '#8C8172',
  '--color-focus': '#6EE7B7',
  '--color-accent': '#F3C876',
  '--color-accent-soft': '#33290F',
  '--color-accent-fill': '#F3C876',
  '--color-on-accent-fill': '#2B1F03',
  '--color-danger': '#FDA29B',
  '--color-on-danger': '#450A0A',
  '--color-error': '#FDA29B',
  '--color-error-bg': '#3B1D1A',
  '--color-warning': '#F3C876',
  '--color-warning-bg': '#3A2E12',
  '--color-info': '#BDB2A1',
  '--color-info-bg': '#28241D',
  '--color-tint-a': '#3A2E12',
  '--color-ink-a': '#F3C876',
  '--color-tint-b': '#16342A',
  '--color-ink-b': '#6EE7B7',
  '--color-tint-c': '#3D2417',
  '--color-ink-c': '#F2AB84',
  '--color-bubble': '#1F4A3B',
  '--color-on-bubble': '#F6F1E8',
  '--color-bubble-meta': '#B9D8CB',
  '--color-hero': '#12382C',
  '--color-hero-deep': '#081F18',
  '--color-hero-text': '#F4FBF7',
  '--color-hero-muted': '#C4D9CE',
  '--color-hero-accent': '#F3C876',
  '--color-hero-border': '#396455',
} as const

async function tokenValues(page: Page, names: string[]) {
  const values = await page.evaluate((tokens) => {
    const styles = getComputedStyle(document.documentElement)
    return Object.fromEntries(tokens.map((name) => [name, styles.getPropertyValue(name).trim()]))
  }, names)
  return Object.fromEntries(Object.entries(values).map(([name, value]) => {
    const shortHex = /^#([\da-f])([\da-f])([\da-f])$/i.exec(value)
    const normalized = shortHex
      ? `#${shortHex.slice(1).map((digit) => digit.repeat(2)).join('')}`
      : value
    return [name, normalized.toUpperCase()]
  }))
}

test('@theme native control, exact semantic tokens, local font, and quality gates', async ({ page }) => {
  const fontURLs: string[] = []
  page.on('response', (response) => {
    if (response.request().resourceType() === 'font') fontURLs.push(response.url())
  })
  await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'light'))
  await page.goto('/')

  const theme = page.getByRole('combobox', { name: 'پوسته', exact: true })
  await expect(theme).toBeVisible()
  await expect(theme).toHaveValue('light')
  await expect(theme.locator('option')).toHaveText(['خودکار', 'روشن', 'تیره'])
  expect(await theme.locator('option').evaluateAll((options) => (
    options.map((option) => option.getAttribute('value'))
  ))).toEqual(['system', 'light', 'dark'])
  expect(await tokenValues(page, Object.keys(LIGHT))).toEqual(LIGHT)
  await expectNoSeriousA11y(page)
  await expectNoOverflow(page)
  await expectTouchTargets(page)

  await theme.selectOption('dark')
  await expect(page.locator('html')).toHaveClass(/(^|\s)dark(\s|$)/)
  expect(await tokenValues(page, Object.keys(DARK))).toEqual(DARK)
  await expectNoSeriousA11y(page)
  await expectNoOverflow(page)
  await expectTouchTargets(page)

  await page.waitForFunction(() => document.documentElement.classList.contains('fonts-enhanced'))
  await page.evaluate(() => document.fonts.ready.then(() => undefined))
  expect(fontURLs.length).toBeGreaterThan(0)
  for (const fontURL of fontURLs) {
    const url = new URL(fontURL)
    expect(url.origin).toBe(new URL(page.url()).origin)
    expect(url.pathname).toMatch(/\.woff2$/)
  }
  expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain('Vazirmatn Variable')
})

test('@theme no-JavaScript head fallback keeps local Vazirmatn', async ({ browser }) => {
  const context = await browser.newContext({
    baseURL: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:3000',
    javaScriptEnabled: false,
  })
  try {
    const page = await context.newPage()
    const fontURLs: string[] = []
    page.on('response', (response) => {
      if (response.request().resourceType() === 'font') fontURLs.push(response.url())
    })
    await page.goto('/')
    await expect(page.locator('main')).toBeVisible()
    await expect(page.locator('head noscript style')).toHaveCount(1)
    expect(await page.locator('body').evaluate((element) => getComputedStyle(element).fontFamily))
      .toContain('Vazirmatn Variable')
    await page.evaluate(() => document.fonts.ready.then(() => undefined))
    expect(fontURLs.length).toBeGreaterThan(0)
    for (const fontURL of fontURLs) {
      expect(new URL(fontURL).origin).toBe(new URL(page.url()).origin)
    }
  } finally {
    await context.close()
  }
})

test('@theme stored dark applies before paint without a light-class flash', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('nuxt-color-mode', 'dark')
    const observed: string[] = []
    Object.assign(window, { __phase7ThemeClasses: observed })
    const observeRoot = () => {
      const root = document.documentElement
      if (!root) return false
      observed.push(root.className)
      new MutationObserver(() => observed.push(root.className)).observe(root, {
        attributes: true,
        attributeFilter: ['class'],
      })
      return true
    }
    if (!observeRoot()) {
      const documentObserver = new MutationObserver(() => {
        if (observeRoot()) documentObserver.disconnect()
      })
      documentObserver.observe(document, { childList: true, subtree: true })
    }
  })
  await page.goto('/')
  await expect(page.locator('html')).toHaveClass(/(^|\s)dark(\s|$)/)
  await expect(page.getByRole('combobox', { name: 'پوسته', exact: true })).toHaveValue('dark')
  const observed = await page.evaluate(() => (
    window as unknown as { __phase7ThemeClasses: string[] }
  ).__phase7ThemeClasses)
  expect(observed.some((classes) => classes.split(/\s+/).includes('light'))).toBe(false)
})

test('@theme system preference resolves dark and remains selected as system', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' })
  await page.addInitScript(() => localStorage.setItem('nuxt-color-mode', 'system'))
  await page.goto('/')
  await expect(page.locator('html')).toHaveClass(/(^|\s)dark(\s|$)/)
  await expect(page.getByRole('combobox', { name: 'پوسته', exact: true })).toHaveValue('system')
  expect((await tokenValues(page, ['--color-bg', '--color-text']))).toEqual({
    '--color-bg': DARK['--color-bg'],
    '--color-text': DARK['--color-text'],
  })
})
