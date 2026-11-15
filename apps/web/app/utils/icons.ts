// Every icon the app draws. `scripts/build-icon-sprite.mjs` builds
// `app/assets/icons/sprite.svg` from this list, and a unit test fails when the two differ
// or when a name here is used nowhere. A name is the Phosphor file name: the weight is its
// suffix (`-fill`, `-bold`, `-duotone`), and no suffix means regular.
export const ICON_NAMES = [
  'house', 'house-fill', 'magnifying-glass', 'plus-circle-fill', 'chat-circle', 'chat-circle-fill',
  'heart', 'heart-fill', 'user-circle', 'user-circle-fill', 'shield-check', 'sun', 'moon',
  'warning-circle-fill', 'check-circle-fill', 'warning-fill', 'info-fill',
] as const

export type AppIconName = typeof ICON_NAMES[number]
