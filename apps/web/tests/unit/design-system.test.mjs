import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { test } from 'node:test'

const WEB = new URL('../../', import.meta.url).pathname
// The whole app is on the new scale since Task 9.
const SCOPE = ['app/']

const files = readdirSync(`${WEB}app`, { recursive: true })
  .map((name) => `app/${name}`)
  .filter((name) => /\.(vue|css)$/.test(name))
const scoped = files.filter((name) => name !== 'app/assets/css/tokens.css' && SCOPE.some((prefix) => name.startsWith(prefix)))
const read = (name) => readFileSync(`${WEB}${name}`, 'utf8')
const css = (name) => (name.endsWith('.css') ? read(name) : [...read(name).matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((match) => match[1]).join('\n'))
  .replace(/\/\*[\s\S]*?\*\//g, '')
const template = (name) => (name.endsWith('.vue') ? read(name).split(/<script[\s>]/)[0] : '')
const values = (name, property) => [...css(name).matchAll(new RegExp(`(?<![\\w-])${property}\\s*:\\s*([^;}]+)`, 'g'))].map((match) => match[1].trim())

const RADIUS = /^(var\(--radius-(small|control|card|panel|dialog|pill)\)|50%|0|inherit)$/
const SHADOW = /^(var\(--(shadow-card|shadow-lifted|glow-primary|glow-accent|ring-focus)\)|none|inherit)$/
const RULES = [
  ['font-weight', (value) => /^(400|600|800|inherit)$/.test(value)],
  ['font-size', (value) => /^(var\(--font-size-(12|14|16|18|22|28|36|display)\)|inherit)$/.test(value)],
  ['font', (value) => value === 'inherit'],
  // The chat bubble's one tight corner is the only raw radius (spec §13.3).
  ['border(?:-[a-z]+)*-radius', (value, name) => value.split(/[\s/]+/).every((part) => RADIUS.test(part) || (part === '6px' && name.endsWith('MessageBubble.vue')))],
  ['box-shadow', (value) => value.split(/,\s*(?![^(]*\))/).every((part) => SHADOW.test(part.trim()))],
]

for (const [property, allowed] of RULES) {
  test(`${property} uses only the scale`, () => {
    const offenders = scoped.flatMap((name) => values(name, property).filter((value) => !allowed(value, name)).map((value) => `${name}: ${value}`))
    assert.deepEqual(offenders, [])
  })
}

test('no glyph or emoji stands in for an icon, and no inline svg outside AppIcon and BrandMascot', () => {
  const offenders = scoped.flatMap((name) => {
    const markup = template(name)
    const glyphs = markup.match(/[★☆→←⋮✓✔]|\p{Extended_Pictographic}/gu) ?? []
    const inlineSvg = /<svg[\s>]/.test(markup) && !/(AppIcon|BrandMascot)\.vue$/.test(name)
    return [...glyphs.map((glyph) => `${name}: ${glyph}`), ...(inlineSvg ? [`${name}: <svg>`] : [])]
  })
  assert.deepEqual(offenders, [])
})
