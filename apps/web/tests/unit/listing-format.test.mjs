import assert from 'node:assert/strict'
import { test } from 'node:test'
import { formatListingDate, formatListingDay } from '../../app/utils/format.ts'
import { categoryIcon } from '../../app/utils/icons.ts'

const NOW = Date.parse('2026-11-14T12:00:00Z') // 15:30 in Tehran (UTC+03:30, no DST)
const cases = [
  ['the same Tehran day', '2026-11-14T08:10:00Z', NOW, 'امروز'],
  ['the first minute of the Tehran day', '2026-11-13T20:30:00Z', NOW, 'امروز'],
  ['the last second of the previous Tehran day', '2026-11-13T20:29:59Z', NOW, 'دیروز'],
  ['two days', '2026-11-12T12:00:00Z', NOW, '۲ روز پیش'],
  ['six days', '2026-11-08T12:00:00Z', NOW, '۶ روز پیش'],
  ['seven days falls back to the date', '2026-11-07T12:00:00Z', NOW, formatListingDate('2026-11-07T12:00:00Z')],
  ['a timestamp ahead of the clock', '2026-11-15T09:00:00Z', NOW, 'امروز'],
  ['31 minutes ago across Tehran midnight', '2026-11-14T20:00:00Z', Date.parse('2026-11-14T20:31:00Z'), 'دیروز'],
  ['an unparseable value', 'not-a-date', NOW, ''],
]
for (const [label, value, now, expected] of cases) {
  test(`day label: ${label}`, () => assert.equal(formatListingDay(value, now), expected))
}

test('category icons cover the seven seeded slugs, the "all" tile and a slug added later', () => {
  const expected = {
    '': 'squares-four-duotone', 'home-sweets': 'cake-duotone', 'home-food': 'bowl-food-duotone',
    handicrafts: 'palette-duotone', 'clothing-sewing': 't-shirt-duotone', 'plants-flowers': 'plant-duotone',
    beauty: 'flower-lotus-duotone', education: 'graduation-cap-duotone', 'a-slug-added-later': 'tag-duotone',
  }
  for (const [slug, icon] of Object.entries(expected)) assert.equal(categoryIcon(slug), icon)
})
