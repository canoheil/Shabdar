<template>
  <div class="bubble" :class="own ? 'bubble--own' : 'bubble--other'">
    <span class="bubble__avatar" :class="own ? 'bubble__avatar--own' : ''" aria-hidden="true">
      {{ initial }}
    </span>
    <div class="bubble__surface">
      <!-- Sender is visible through the avatar and the message side; screen
           readers get it as text instead of a colour or position alone. -->
      <span class="sr-only">{{ own ? 'شما' : authorName }}:</span>
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
  // Display name of the sender, used for the avatar initial and the
  // screen-reader-only sender label.
  authorName: string
}>()

const time = computed(() => formatChatTime(props.message.created_at))
const initial = computed(() => {
  const name = props.authorName.trim()
  return name ? Array.from(name)[0] ?? 'ش' : 'ش'
})
</script>

<style scoped>
.bubble {
  display: flex;
  align-items: flex-end;
  gap: var(--space-2);
  justify-content: flex-start;
  margin: 0;
  list-style: none;
}

.bubble--own {
  flex-direction: row-reverse;
  justify-content: flex-start;
}

.bubble__avatar {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  inline-size: 2rem;
  block-size: 2rem;
  font-size: 0.8125rem;
  font-weight: 700;
  color: var(--color-primary);
  background: color-mix(in srgb, var(--color-primary) 12%, var(--color-surface));
  border: 1px solid color-mix(in srgb, var(--color-primary) 26%, var(--color-border));
  border-radius: 999px;
  user-select: none;
}

.bubble__avatar--own {
  color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent) 12%, var(--color-surface));
  border-color: color-mix(in srgb, var(--color-accent) 26%, var(--color-border));
}

.bubble__surface {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  inline-size: fit-content;
  max-inline-size: min(75%, 34rem);
  padding: var(--space-2) var(--space-3);
  color: var(--color-text);
  background: var(--color-surface-muted);
  border: 1px solid var(--color-border);
  /* Tail corner points at the sender, like a standard messenger bubble. */
  border-start-start-radius: 16px;
  border-start-end-radius: 16px;
  border-end-end-radius: 16px;
  border-end-start-radius: 4px;
}

.bubble--own .bubble__surface {
  background: color-mix(in srgb, var(--color-primary) 15%, var(--color-surface));
  border-color: color-mix(in srgb, var(--color-primary) 32%, var(--color-border));
  border-end-start-radius: 16px;
  border-end-end-radius: 4px;
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
