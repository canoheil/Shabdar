/**
 * Chat-only draft rules. Kept beside the chat formatters so the composer and its
 * unit test share one predicate instead of two drifting copies.
 */

// Zero-width and bidirectional controls are invisible to the reader, so a draft
// made only of them is empty for every practical purpose. The API trims and
// rejects the same input; this keeps the doomed request from ever leaving.
const INVISIBLE_CHARACTERS = /[\u200b-\u200f\u202a-\u202e\u2060-\u2064\ufeff]/g

/// True when the draft still has at least one visible character after removing
/// whitespace and zero-width/bidi controls. Length bounds stay in the composer:
/// the server counts raw scalars, the predicate only answers "is there text?".
export function isSendableDraft(value: string): boolean {
  return value.replace(INVISIBLE_CHARACTERS, '').trim().length > 0
}
