<template>
  <article class="listing-card">
    <NuxtLink :to="`/listings/${listing.slug}`" class="listing-card__link">
      <span class="listing-card__media">
        <img
          v-if="showImage"
          ref="imageElement"
          class="listing-card__image"
          :src="listing.cover_image ?? undefined"
          alt=""
          width="480"
          height="360"
          :loading="imagePriority ? 'eager' : 'lazy'"
          :fetchpriority="imagePriority ? 'high' : 'auto'"
          @error="markImageFailed"
        >
        <span v-else class="listing-card__placeholder" aria-hidden="true">
          {{ imageFailed ? 'تصویر در دسترس نیست' : 'بدون تصویر' }}
        </span>
      </span>
      <h3 class="listing-card__title">{{ listing.title }}</h3>
    </NuxtLink>
    <p class="listing-card__price">{{ formatPrice(listing.price_toman, listing.price_type) }}</p>
    <p class="listing-card__meta">{{ listing.city }} · {{ listing.category }}</p>
    <div v-if="$slots.actions" class="listing-card__actions">
      <slot name="actions" />
    </div>
  </article>
</template>

<script setup lang="ts">
import type { ApiListingCard } from '~/types'

const props = withDefaults(defineProps<{
  listing: ApiListingCard
  imagePriority?: boolean
}>(), {
  imagePriority: false,
})

const failedImageId = ref<string | null>(null)
const imageElement = ref<HTMLImageElement | null>(null)
watch(() => props.listing.public_id, () => {
  failedImageId.value = null
})

function checkHydratedImage(): void {
  const image = imageElement.value
  // An SSR image can fail before Vue attaches its error listener.
  if (image?.complete && image.naturalWidth === 0) markImageFailed()
}
onMounted(checkHydratedImage)

const imageFailed = computed(() => failedImageId.value === props.listing.public_id)
const showImage = computed(() => Boolean(props.listing.cover_image) && !imageFailed.value)
function markImageFailed(): void {
  failedImageId.value = props.listing.public_id
}
</script>

<style scoped>
.listing-card {
  display: flex;
  flex-direction: column;
  min-inline-size: 0;
  block-size: 100%;
  padding: var(--space-3);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.listing-card__link {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
  color: var(--color-text);
  text-decoration: none;
  border-radius: var(--radius-control);
}

.listing-card__media {
  display: flex;
  align-items: center;
  justify-content: center;
  inline-size: 100%;
  aspect-ratio: 4 / 3;
  overflow: hidden;
  color: var(--color-text-muted);
  text-align: center;
  background: var(--color-surface-muted);
  border-radius: var(--radius-control);
}

.listing-card__image {
  display: block;
  inline-size: 100%;
  block-size: 100%;
  object-fit: cover;
}

.listing-card__placeholder {
  padding: var(--space-3);
  font-size: 0.875rem;
}

.listing-card__title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  line-height: 1.65;
  overflow-wrap: anywhere;
}

.listing-card__link:hover .listing-card__title {
  color: var(--color-primary);
}

.listing-card__price {
  margin: var(--space-3) 0 0;
  font-weight: 700;
  color: var(--color-primary);
}

.listing-card__meta {
  margin: var(--space-1) 0 0;
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.listing-card__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-block-start: var(--space-3);
}
</style>
