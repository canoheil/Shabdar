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
    <div class="app-alert__body">
      <slot />
    </div>
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

const root = ref<HTMLElement | null>(null)
function focus(): void {
  root.value?.focus()
}
defineExpose({ focus })
</script>
