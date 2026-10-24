import { expect, test, type Page } from '@playwright/test'
import {
  expectNoOverflow,
  expectNoSeriousA11y,
  expectTouchTargets,
} from './support/quality'

const LIGHT = {
  '--color-bg': '#F6F8F7',
  '--color-surface': '#FFFFFF',
  '--color-surface-muted': '#EDF2EF',
  '--color-text': '#17211B',
  '--color-text-muted': '#52615A',
  '--color-primary': '#047857',
  '--color-on-primary': '#FFFFFF',
  '--color-primary-hover': '#065F46',
  '--color-border': '#D5DEDA',
  '--color-control-border': '#68776E',
  '--color-focus': '#047857',
  '--color-danger': '#B91C1C',
  '--color-on-danger': '#FFFFFF',
  '--color-warning': '#92400E',
  '--color-warning-bg': '#FFFBEB',
  '--color-info': '#075985',
  '--color-info-bg': '#F0F9FF',
  '--color-error': '#991B1B',
  '--color-error-bg': '#FEF2F2',
} as const

const DARK = {
  '--color-bg': '#0E1511',
  '--color-surface': '#162019',
  '--color-surface-muted': '#202D25',
  '--color-text': '#F2F6F3',
  '--color-text-muted': '#B2C0B6',
  '--color-primary': '#6EE7B7',
  '--color-on-primary': '#062E22',
  '--color-primary-hover': '#A7F3D0',
  '--color-border': '#34463B',
  '--color-control-border': '#7E9184',
  '--color-focus': '#6EE7B7',
  '--color-danger': '#FCA5A5',
  '--color-on-danger': '#450A0A',
  '--color-warning': '#FBBF24',
  '--color-warning-bg': '#2B2111',
  '--color-info': '#7DD3FC',
  '--color-info-bg': '#10202B',
  '--color-error': '#FCA5A5',
  '--color-error-bg': '#162019',
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

  await page.evaluate(() => document.fonts.ready.then(() => undefined))
  expect(fontURLs.length).toBeGreaterThan(0)
  for (const fontURL of fontURLs) {
    const url = new URL(fontURL)
    expect(url.origin).toBe(new URL(page.url()).origin)
    expect(url.pathname).toMatch(/\.woff2$/)
  }
  expect(await page.evaluate(() => getComputedStyle(document.body).fontFamily)).toContain('Vazirmatn Variable')
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
