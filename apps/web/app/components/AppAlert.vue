<template>
  <div
    ref="root"
    class="app-alert"
    :class="`app-alert--${tone}`"
    :data-tone="tone"
    :role="tone === 'error' ? 'alert' : 'status'"
    tabindex="-1"
  >
    <AppIcon :name="TONE_ICON[tone]" />
    <slot />
  </div>
</template>

<script setup lang="ts">
import type { AppIconName } from '~/utils/icons'

type Tone = 'error' | 'success' | 'info' | 'warning'

// A status is never a colour alone: each tone has its own glyph beside the words.
const TONE_ICON: Record<Tone, AppIconName> = {
  error: 'warning-circle-fill',
  success: 'check-circle-fill',
  warning: 'warning-fill',
  info: 'info-fill',
}

withDefaults(defineProps<{
  tone: Tone
}>(), {
  tone: 'info',
})

// The slot stays a direct child of the root, with no wrapper around it. The root takes focus
// after a failure, and comments.spec.ts finds it by its exact text: a wrapper would be found
// in its place. CSS alone puts the icon beside the words.
const root = ref<HTMLElement | null>(null)
function focus(): void {
  root.value?.focus()
}
defineExpose({ focus })
</script>
