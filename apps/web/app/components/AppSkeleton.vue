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
