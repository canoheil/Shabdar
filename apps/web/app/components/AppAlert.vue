<template>
  <div
    ref="root"
    class="app-alert"
    :class="`app-alert--${tone}`"
    :data-tone="tone"
    :role="tone === 'error' ? 'alert' : 'status'"
    tabindex="-1"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  tone: 'error' | 'success' | 'info' | 'warning'
}>(), {
  tone: 'info',
})

const root = ref<HTMLElement | null>(null)
function focus(): void {
  root.value?.focus()
}
defineExpose({ focus })
</script>
