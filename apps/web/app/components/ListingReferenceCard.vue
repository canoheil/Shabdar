<template>
  <NuxtLink
    v-if="reference.available && reference.slug"
    :to="`/listings/${reference.slug}`"
    class="listing-reference"
    :aria-label="`آگهی مرتبط: ${title}`"
  >
    <span class="listing-reference__label">آگهی مرتبط</span>
    <span class="listing-reference__title">{{ title }}</span>
  </NuxtLink>
  <div v-else class="listing-reference listing-reference--unavailable">
    <span class="listing-reference__label">آگهی مرتبط</span>
    <span class="listing-reference__title">{{ title }}</span>
    <span class="listing-reference__note">این آگهی دیگر در دسترس نیست.</span>
  </div>
</template>

<script setup lang="ts">
import type { ApiListingReference } from '~/types'

const props = defineProps<{
  reference: ApiListingReference
}>()

// Removed listings keep their stored title as plain text; only an active,
// public reference is a navigable target.
const title = computed(() => props.reference.title?.trim() || 'آگهی بدون عنوان')
</script>

<style scoped>
.listing-reference {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-2) var(--space-3);
  font-size: 0.8125rem;
  line-height: 1.5;
  color: inherit;
  text-decoration: none;
  background: color-mix(in srgb, var(--color-surface) 82%, transparent);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

a.listing-reference:hover {
  border-color: var(--color-primary);
}

a.listing-reference:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

.listing-reference__label {
  font-size: 0.6875rem;
  font-weight: 600;
  color: var(--color-text-muted);
}

.listing-reference__title {
  font-weight: 600;
  overflow-wrap: anywhere;
}

.listing-reference--unavailable .listing-reference__title {
  color: var(--color-text-muted);
}

.listing-reference__note {
  font-size: 0.6875rem;
  color: var(--color-text-muted);
}
</style>
