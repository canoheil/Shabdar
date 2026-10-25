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
