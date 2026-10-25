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
