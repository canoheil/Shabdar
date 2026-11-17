// Every icon the app draws. `scripts/build-icon-sprite.mjs` builds
// `app/assets/icons/sprite.svg` from this list, and a unit test fails when the two differ
// or when a name here is used nowhere. A name is the Phosphor file name: the weight is its
// suffix (`-fill`, `-bold`, `-duotone`), and no suffix means regular.
export const ICON_NAMES = [
  'house', 'house-fill', 'magnifying-glass', 'magnifying-glass-bold', 'plus-circle-fill', 'chat-circle', 'chat-circle-fill',
  'heart', 'heart-fill', 'user-circle', 'user-circle-fill', 'shield-check', 'sun', 'moon',
  'warning-circle-fill', 'check-circle-fill', 'warning-fill', 'info-fill',
  'squares-four-duotone', 'cake-duotone', 'bowl-food-duotone', 'palette-duotone',
  't-shirt-duotone', 'plant-duotone', 'flower-lotus-duotone', 'graduation-cap-duotone',
  'tag-duotone', 'shield-check-duotone', 'chat-circle-duotone', 'flag-duotone',
  'map-pin', 'map-pin-fill', 'star-fill', 'star', 'x-bold', 'sliders-horizontal-bold',
  'arrow-right-bold', 'caret-down-bold', 'caret-right-bold', 'caret-left-bold', 'dots-three-bold',
  'camera-plus-duotone', 'phone-duotone', 'clipboard-text-duotone',
  'check-circle-duotone', 'clock-duotone', 'heart-duotone', 'clock-fill', 'x-circle-fill', 'minus-circle-fill',
  'dots-three-vertical-bold', 'paper-plane-right-fill', 'checks-bold', 'tag',
] as const

export type AppIconName = typeof ICON_NAMES[number]

const CATEGORY_ICONS: Record<string, AppIconName> = {
  '': 'squares-four-duotone',
  'home-sweets': 'cake-duotone',
  'home-food': 'bowl-food-duotone',
  'handicrafts': 'palette-duotone',
  'clothing-sewing': 't-shirt-duotone',
  'plants-flowers': 'plant-duotone',
  'beauty': 'flower-lotus-duotone',
  'education': 'graduation-cap-duotone',
}

export function categoryIcon(slug: string): AppIconName {
  return CATEGORY_ICONS[slug] ?? 'tag-duotone'
}
