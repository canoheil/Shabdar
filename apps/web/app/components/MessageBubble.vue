<template>
  <div class="bubble" :class="own ? 'bubble--own' : 'bubble--other'">
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
        <span v-if="own && seen" class="bubble__seen"><AppIcon name="checks-bold" :size="13" /><span class="sr-only">دیده شد</span></span>
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
</script>

<style scoped>
.bubble {
  display: flex;
  /* Sent and received messages are told apart by side, not only by colour. The
     row always fills the scroller, so the two sides are far apart even when one
     message is short. */
  inline-size: 100%;
  align-items: flex-end;
  gap: var(--space-2);
  justify-content: flex-start;
  margin: 0;
  list-style: none;
}

/* RTL: `flex-start` is the right edge, so the other party's message sits on the
   right and the viewer's own message on the left. The auto margins state that
   intent explicitly, so the side survives a change of flex direction and cannot
   be inferred from the source order alone (a structural test reads these edges). */
.bubble:not(.bubble--own) {
  margin-inline-end: auto;
}

.bubble--own {
  flex-direction: row-reverse;
  margin-inline-start: auto;
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
  /* Tail corner points at the sender, like a standard messenger bubble. */
  border-start-start-radius: var(--radius-card);
  border-start-end-radius: var(--radius-card);
  border-end-end-radius: var(--radius-card);
  border-end-start-radius: 6px;
}

.bubble--own .bubble__surface {
  color: var(--color-on-bubble);
  background: var(--color-bubble);
  border-end-start-radius: var(--radius-card);
  border-end-end-radius: 6px;
}

.bubble__body {
  margin: 0;
  font-size: var(--font-size-16);
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
  font-size: var(--font-size-12);
  color: var(--color-text-muted);
}

.bubble--own .bubble__meta {
  color: var(--color-bubble-meta);
}

.bubble__time {
  font-variant-numeric: tabular-nums;
}

.bubble__seen {
  display: inline-flex;
  align-items: center;
  color: var(--color-bubble-meta);
}

.bubble__listing {
  margin-block-end: var(--space-1);
}
</style>
