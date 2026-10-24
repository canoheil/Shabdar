<template>
  <div class="app-skeleton" :class="`app-skeleton--${variant}`" role="status" aria-live="polite">
    <span class="sr-only">{{ label }}</span>
    <div v-for="index in count" :key="index" class="app-skeleton__item" aria-hidden="true">
      <div v-if="variant !== 'text'" class="app-skeleton__media" />
      <div class="app-skeleton__line app-skeleton__line--title" />
      <div class="app-skeleton__line" />
      <div v-if="variant === 'detail'" class="app-skeleton__line app-skeleton__line--short" />
    </div>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  variant: 'card' | 'text' | 'detail'
  count?: number
  label?: string
}>(), {
  count: 1,
  label: 'در حال بارگذاری…',
})
</script>

<style scoped>
.app-skeleton {
  display: grid;
  gap: var(--space-4);
}

.app-skeleton--card {
  grid-template-columns: repeat(auto-fill, minmax(min(15rem, 100%), 1fr));
}

.app-skeleton__item {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.app-skeleton__media {
  inline-size: 100%;
  aspect-ratio: 4 / 3;
  background: var(--color-surface-muted);
  border-radius: var(--radius-control);
}

.app-skeleton--detail .app-skeleton__media {
  aspect-ratio: 4 / 3;
}

.app-skeleton__line {
  inline-size: 100%;
  block-size: 1rem;
  background: var(--color-surface-muted);
  border-radius: var(--radius-control);
}

.app-skeleton__line--title {
  inline-size: 78%;
  block-size: 1.25rem;
}

.app-skeleton__line--short {
  inline-size: 48%;
}
</style>
