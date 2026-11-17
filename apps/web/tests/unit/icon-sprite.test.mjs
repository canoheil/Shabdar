import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { test } from 'node:test'
import { ICON_NAMES } from '../../app/utils/icons.ts'

const WEB = new URL('../../', import.meta.url).pathname
const sprite = readFileSync(`${WEB}app/assets/icons/sprite.svg`, 'utf8')
const symbols = [...sprite.matchAll(/<symbol id="([a-z0-9-]+)" viewBox="0 0 256 256">/g)].map((match) => match[1])

test('the sprite holds exactly the declared icons', () => {
  assert.deepEqual(symbols, [...ICON_NAMES].sort())
})

test('every declared icon is written as a literal somewhere in app/', () => {
  const sources = readdirSync(`${WEB}app`, { recursive: true })
    .filter((name) => /\.(vue|ts)$/.test(name))
    .map((name) => readFileSync(`${WEB}app/${name}`, 'utf8'))
    .join('\n')
  // One occurrence is the declaration in icons.ts, so a used icon appears at least twice.
  const unused = ICON_NAMES.filter((name) => sources.split(new RegExp(`['"\`]${name}['"\`]`)).length < 3)
  assert.deepEqual(unused, [])
})

test('the sprite is plain local geometry under 48 KB', () => {
  assert.doesNotMatch(sprite, /<(script|style|image|foreignObject)\b|href=|url\(/i)
  assert.ok(Buffer.byteLength(sprite) < 48_000)
})
