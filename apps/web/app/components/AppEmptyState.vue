<template>
  <section class="app-empty-state">
    <BrandMascot
      v-if="mascot !== 'none'"
      :pose="mascot"
      :decorative="true"
      class="app-empty-state__mascot"
    />
    <h2 class="app-empty-state__title">{{ title }}</h2>
    <p v-if="description" class="app-empty-state__description">{{ description }}</p>
    <div v-if="$slots.default" class="app-empty-state__actions">
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  title: string
  description?: string
  mascot?: 'none' | 'full' | 'search'
}>(), {
  description: undefined,
  mascot: 'none',
})
</script>

<style scoped>
.app-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8) var(--space-4);
  text-align: center;
  background: var(--color-surface);
  border: 1px dashed var(--color-control-border);
  border-radius: var(--radius-card);
}

.app-empty-state__mascot {
  inline-size: 88px;
  block-size: 88px;
}

.app-empty-state__title {
  margin: 0;
  font-size: 1.125rem;
}

.app-empty-state__description {
  max-inline-size: 34rem;
  margin: 0;
  color: var(--color-text-muted);
}

.app-empty-state__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--space-2);
}
</style>
