import assert from 'node:assert/strict'
import { test } from 'node:test'
import { isSendableDraft } from '../../app/utils/chatDraft.ts'

// The API trims and rejects bodies without a visible character. The composer
// must agree, so an empty or ZWNJ-only draft never becomes a doomed request.
const rejects = [
  ['empty', ''],
  ['spaces', '   '],
  ['zwnj only', '\u200c'],
  ['zwnj and spaces', '\u200c\u200c  '],
  ['directional mark', '\u200f'],
  ['newline and tab', '\n\t '],
  ['zero width space', '\u200b'],
  ['bom', '\ufeff'],
  ['mixed invisibles', '\u200b\u200c\u202e \n'],
]

for (const [label, value] of rejects) {
  test(`rejects ${label}`, () => {
    assert.equal(isSendableDraft(value), false)
  })
}

const accepts = [
  ['persian', 'سلام'],
  ['padded latin', ' a '],
  ['emoji', '🎉'],
  ['two lines', 'خط اول\nخط دوم'],
  ['zwnj inside a word', 'می\u200cخواهم'],
  ['digits', '۱۲۳'],
]

for (const [label, value] of accepts) {
  test(`accepts ${label}`, () => {
    assert.equal(isSendableDraft(value), true)
  })
}

test('accepts a body at the 1000-character bound', () => {
  assert.equal(isSendableDraft('ا'.repeat(1000)), true)
})

test('the predicate is length-agnostic; the composer owns the 1000 bound', () => {
  assert.equal(isSendableDraft('ا'.repeat(1001)), true)
})

test('accepts a visible character hidden among invisibles', () => {
  assert.equal(isSendableDraft('\u200c \u200f ب \u200c'), true)
})
