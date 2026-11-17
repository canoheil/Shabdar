<template>
  <li class="conversation-row">
    <NuxtLink :to="href" class="conversation-row__link">
      <span class="avatar conversation-row__avatar" aria-hidden="true">{{ initial }}</span>
      <span class="conversation-row__main">
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
      <span v-if="listingNote" class="conversation-row__listing"><AppIcon name="tag" :size="12" /> {{ listingNote }}</span>
      </span>
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
const initial = computed(() => Array.from(name.value)[0] ?? 'ش')
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
  align-items: flex-start;
  gap: var(--space-3);
  min-block-size: 44px;
  padding: var(--space-3);
  color: var(--color-text);
  text-decoration: none;
}

.conversation-row__main {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-1);
  min-inline-size: 0;
}

.conversation-row__avatar {
  inline-size: 44px;
  block-size: 44px;
}

.conversation-row:has(.conversation-row__unread) .conversation-row__preview {
  font-weight: 600;
  color: var(--color-text);
}

.conversation-row:has(.conversation-row__unread) .conversation-row__time {
  color: var(--color-primary);
}

.conversation-row__head,
.conversation-row__body {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
}

.conversation-row__name {
  font-weight: 800;
  overflow-wrap: anywhere;
}

.conversation-row__time {
  flex: 0 0 auto;
  font-size: var(--font-size-12);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.conversation-row__preview {
  display: -webkit-box;
  overflow: hidden;
  font-size: var(--font-size-14);
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
  font-size: var(--font-size-12);
  font-weight: 800;
  text-align: center;
  color: var(--color-on-accent-fill);
  background: var(--color-accent-fill);
  border-radius: var(--radius-pill);
}

.conversation-row__listing {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--font-size-12);
  color: var(--color-text-muted);
  overflow-wrap: anywhere;
}
</style>
