<template>
  <li class="conversation-row">
    <NuxtLink :to="href" class="conversation-row__link">
      <span class="conversation-row__head">
        <span class="conversation-row__name">{{ name }}</span>
        <time class="conversation-row__time" :datetime="item.last_message_at">
          {{ time }}
        </time>
      </span>
      <span class="conversation-row__body">
        <span class="conversation-row__preview">
          <span v-if="previewPrefix" class="conversation-row__prefix">{{ previewPrefix }}</span>
          {{ item.last_message.body_preview }}
        </span>
        <span
          v-if="item.unread_count > 0"
          class="conversation-row__unread"
          :aria-label="`${formatNumber(item.unread_count)} پیام خوانده‌نشده`"
        >
          {{ formatNumber(item.unread_count) }}
        </span>
      </span>
      <span v-if="listingNote" class="conversation-row__listing">{{ listingNote }}</span>
    </NuxtLink>
  </li>
</template>

<script setup lang="ts">
import type { ApiInboxItem } from '~/types'

const props = defineProps<{
  item: ApiInboxItem
  viewerPublicId: string
}>()

const href = computed(() => `/me/conversations/${props.item.public_id}`)
const name = computed(() => props.item.other_party.display_name?.trim() || 'کاربر شبدر')
const time = computed(() => formatChatTime(props.item.last_message_at))
const previewPrefix = computed(() =>
  props.item.last_message.sender_public_id === props.viewerPublicId ? 'شما:' : '',
)
const listingNote = computed(() => {
  const listing = props.item.last_message.listing
  if (!listing?.title) return ''
  return listing.available ? `دربارهٔ «${listing.title}»` : `دربارهٔ «${listing.title}» (حذف‌شده)`
})
</script>

<style scoped>
.conversation-row {
  margin: 0;
  list-style: none;
}

.conversation-row__link {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-block-size: 44px;
  padding: var(--space-3);
  color: var(--color-text);
  text-decoration: none;
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

.conversation-row__link:hover,
.conversation-row__link:focus-visible {
  border-color: var(--color-primary);
}

.conversation-row__head,
.conversation-row__body {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
}

.conversation-row__name {
  font-weight: 700;
  overflow-wrap: anywhere;
}

.conversation-row__time {
  flex: 0 0 auto;
  font-size: 0.75rem;
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.conversation-row__preview {
  display: -webkit-box;
  overflow: hidden;
  font-size: 0.875rem;
  color: var(--color-text-muted);
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
}

.conversation-row__prefix {
  font-weight: 600;
}

.conversation-row__unread {
  flex: 0 0 auto;
  min-inline-size: 1.5rem;
  padding-inline: var(--space-1);
  font-size: 0.75rem;
  font-weight: 700;
  text-align: center;
  color: var(--color-on-primary);
  background: var(--color-primary);
  border-radius: 999px;
}

.conversation-row__listing {
  font-size: 0.75rem;
  color: var(--color-text-muted);
  overflow-wrap: anywhere;
}
</style>
