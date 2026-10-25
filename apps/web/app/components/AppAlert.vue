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

<style scoped>
.app-alert {
  padding: var(--space-3) var(--space-4);
  border: 1px solid currentColor;
  border-radius: var(--radius-control);
}

.app-alert--error {
  color: var(--color-error);
  background: var(--color-error-bg);
}

.app-alert--success {
  color: var(--color-primary);
  background: var(--color-surface-muted);
}

.app-alert--info {
  color: var(--color-info);
  background: var(--color-info-bg);
}

.app-alert--warning {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}
</style>
