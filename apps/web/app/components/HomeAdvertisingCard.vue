<template>
  <aside class="home-ad" aria-labelledby="home-ad-title" data-advertising-card>
    <div class="home-ad__topline">
      <span class="home-ad__label">فرصت تبلیغاتی</span>
      <button
        v-if="dismissible"
        type="button"
        class="home-ad__dismiss"
        aria-label="بستن فرصت تبلیغاتی"
        @click="$emit('dismiss')"
      >
        <AppIcon name="x-bold" :size="16" />
        <span class="sr-only">بستن</span>
      </button>
      <span v-else class="home-ad__preview">پیش‌نمایش</span>
    </div>
    <BrandMascot v-if="internal" pose="full" class="home-ad__mascot" />
    <h3 id="home-ad-title">{{ slot.title }}</h3>
    <p>{{ slot.body }}</p>
    <NuxtLink v-if="internal" :to="destination" class="home-ad__cta">
      {{ slot.cta_label }}
    </NuxtLink>
    <a v-else :href="destination" class="home-ad__cta" rel="sponsored">
      {{ slot.cta_label }}
    </a>
  </aside>
</template>

<script setup lang="ts">
import type { ApiAdvertisingSlot } from '~/types'

const props = withDefaults(defineProps<{
  slot: ApiAdvertisingSlot
  dismissible?: boolean
}>(), {
  dismissible: true,
})

defineEmits<{
  dismiss: []
}>()

const destination = computed(() => {
  const value = props.slot.cta_url
  if (/^\/(?!\/)[^\s\\]*$/.test(value) || /^https:\/\/[^\s\\@]+$/.test(value)) return value
  // Admin preview can contain an unsaved invalid draft; never render it as a link.
  return '/advertising'
})
const internal = computed(() => destination.value.startsWith('/'))
</script>

<style scoped>
.home-ad {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-3);
  block-size: 100%;
  padding: var(--space-5);
  background: var(--color-accent-soft);
  border: 1px dashed var(--color-primary);
  border-radius: var(--radius-card);
}

.home-ad__topline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  inline-size: 100%;
}

.home-ad__label,
.home-ad__preview {
  color: var(--color-primary);
  font-size: var(--font-size-12);
  font-weight: 800;
}

.home-ad__dismiss {
  min-block-size: 44px;
  padding-inline: var(--space-2);
  color: var(--color-text-muted);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.home-ad__mascot {
  inline-size: 56px;
  block-size: 56px;
  color: var(--color-accent);
}

.home-ad h3,
.home-ad p {
  margin: 0;
}

.home-ad h3 {
  font-size: var(--font-size-18);
  line-height: 1.7;
}

.home-ad p {
  flex: 1;
  color: var(--color-text-muted);
  font-size: var(--font-size-14);
  line-height: 1.9;
  white-space: pre-line;
}

.home-ad__cta {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
  color: var(--color-primary);
  font-weight: 800;
  text-decoration: underline;
  text-underline-offset: 0.25rem;
}
</style>
