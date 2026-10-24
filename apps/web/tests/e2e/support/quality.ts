import AxeBuilder from '@axe-core/playwright'
import { expect, type Page } from '@playwright/test'

export async function expectNoSeriousA11y(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page }).analyze()
  const violations = results.violations
    .filter((violation) => violation.impact === 'serious' || violation.impact === 'critical')
    .map((violation) => ({
      id: violation.id,
      impact: violation.impact,
      help: violation.help,
      targets: violation.nodes.map((node) => node.target),
    }))
  expect(violations, 'No serious or critical axe violations').toEqual([])
}

export async function expectNoOverflow(page: Page): Promise<void> {
  const dimensions = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }))
  expect(
    dimensions.scrollWidth,
    `Document must not overflow horizontally (${dimensions.scrollWidth} > ${dimensions.clientWidth})`,
  ).toBeLessThanOrEqual(dimensions.clientWidth + 1)
}

export async function expectTouchTargets(page: Page): Promise<void> {
  const offenders = await page.evaluate(() => {
    const selector = [
      'a[href]',
      'button',
      'input:not([type="hidden"])',
      'select',
      'textarea',
      '[role="button"]',
      '[tabindex]:not([tabindex="-1"])',
    ].join(',')
    return [...document.querySelectorAll<HTMLElement>(selector)]
      .filter((element) => {
        if (element.matches(':disabled') || element.closest('[hidden], [aria-hidden="true"]')) return false
        const style = getComputedStyle(element)
        const rect = element.getBoundingClientRect()
        return style.display !== 'none' && style.visibility !== 'hidden' && rect.width > 0 && rect.height > 0
      })
      .map((element) => {
        const rect = element.getBoundingClientRect()
        return {
          element: element.tagName.toLowerCase(),
          label: element.getAttribute('aria-label') || element.textContent?.trim().slice(0, 80) || element.getAttribute('name') || '',
          width: rect.width,
          height: rect.height,
        }
      })
      .filter((target) => target.width < 44 || target.height < 44)
  })
  expect(offenders, 'Every visible interactive target must be at least 44×44 CSS px').toEqual([])
}
