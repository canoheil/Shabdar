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
        بستن
      </button>
      <span v-else class="home-ad__preview">پیش‌نمایش</span>
    </div>
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
  background: var(--color-surface);
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
  font-size: 0.75rem;
  font-weight: 700;
}

.home-ad__dismiss {
  min-block-size: 44px;
  padding-inline: var(--space-2);
  color: var(--color-text-muted);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.home-ad h3,
.home-ad p {
  margin: 0;
}

.home-ad h3 {
  font-size: 1.05rem;
  line-height: 1.7;
}

.home-ad p {
  flex: 1;
  color: var(--color-text-muted);
  font-size: 0.875rem;
  line-height: 1.9;
  white-space: pre-line;
}

.home-ad__cta {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
  color: var(--color-primary);
  font-weight: 700;
  text-decoration: underline;
  text-underline-offset: 0.25rem;
}
</style>
