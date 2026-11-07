<template>
  <div class="bubble" :class="own ? 'bubble--own' : 'bubble--other'">
    <div class="bubble__surface">
      <ListingReferenceCard
        v-if="message.listing"
        :reference="message.listing"
        class="bubble__listing"
      />
      <p class="bubble__body">{{ message.body }}</p>
      <p class="bubble__meta">
        <time class="bubble__time" :datetime="message.created_at">{{ time }}</time>
        <span v-if="own && seen" class="bubble__seen">دیده شد</span>
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ApiMessage } from '~/types'

const props = defineProps<{
  message: ApiMessage
  own: boolean
  // Derived from the conversation's other high-water mark, so a poll that only
  // moves Seen still updates already-rendered own bubbles.
  seen: boolean
}>()

const time = computed(() => formatChatTime(props.message.created_at))
</script>

<style scoped>
.bubble {
  display: flex;
  justify-content: flex-start;
  margin: 0;
  list-style: none;
}

.bubble--own {
  justify-content: flex-end;
}

.bubble__surface {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  inline-size: fit-content;
  max-inline-size: min(85%, 34rem);
  padding: var(--space-2) var(--space-3);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 14px;
}

.bubble--own .bubble__surface {
  background: color-mix(in srgb, var(--color-primary) 9%, var(--color-surface));
  border-color: color-mix(in srgb, var(--color-primary) 26%, var(--color-border));
}

.bubble__body {
  margin: 0;
  font-size: 0.9375rem;
  line-height: 1.75;
  /* Typed newlines are meaningful and long tokens must not overflow the bubble. */
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.bubble__meta {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-2);
  margin: 0;
  font-size: 0.6875rem;
  color: var(--color-text-muted);
}

.bubble__time {
  font-variant-numeric: tabular-nums;
}

.bubble__seen {
  font-weight: 600;
  color: var(--color-primary);
}

.bubble__listing {
  margin-block-end: var(--space-1);
}
</style>
