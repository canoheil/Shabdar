<template>
  <article class="listing-card">
    <NuxtLink :to="`/listings/${listing.slug}`" class="listing-card__link">
      <span class="listing-card__media">
        <picture
          v-if="listing.cover_image && !imageFailed && listing.cover_webp_srcset"
          class="listing-card__picture"
        >
          <source
            type="image/webp"
            :srcset="listing.cover_webp_srcset"
            sizes="(min-width: 1440px) 280px, (min-width: 1024px) 25vw, (min-width: 600px) 33vw, calc(100vw - 64px)"
          >
          <img
            ref="imageElement"
            class="listing-card__image"
            :src="listing.cover_image ?? undefined"
            v-bind="jpegSrcsetAttributes"
            alt=""
            width="480"
            height="360"
            :loading="imagePriority ? 'eager' : 'lazy'"
            :fetchpriority="imagePriority ? 'high' : 'low'"
            @error="markImageFailed"
          >
        </picture>
        <img
          v-else-if="listing.cover_image && !imageFailed"
          ref="imageElement"
          class="listing-card__image"
          :src="listing.cover_image"
          v-bind="jpegSrcsetAttributes"
          alt=""
          width="480"
          height="360"
          :loading="imagePriority ? 'eager' : 'lazy'"
          :fetchpriority="imagePriority ? 'high' : 'low'"
          @error="markImageFailed"
        >
        <span v-else class="listing-card__placeholder" aria-hidden="true">
          <span class="listing-card__placeholder-mark" />
          <span>{{ imageFailed ? 'تصویر در دسترس نیست' : 'بدون تصویر' }}</span>
        </span>
        <span
          v-if="listing.rating_count > 0"
          class="listing-card__rating"
          aria-hidden="true"
        >
          <span class="listing-card__rating-star">★</span>
          {{ formatNumber(Math.round(listing.rating_average * 10) / 10) }}
        </span>
      </span>
      <h3 class="listing-card__title">{{ listing.title }}</h3>
    </NuxtLink>
    <p class="listing-card__price">{{ formatPrice(listing.price_toman, listing.price_type) }}</p>
    <p class="listing-card__meta">
      <NuxtLink
        v-if="cityLink && cityHref"
        :to="cityHref"
        class="listing-card__city-link"
      >
        {{ listing.city }}
      </NuxtLink>
      <span v-else>{{ listing.city }}</span>
      <span aria-hidden="true">·</span>
      <span>{{ listing.category }}</span>
    </p>
    <p class="listing-card__context">
      <time :datetime="listing.published_at">{{ formatListingDate(listing.published_at) }}</time>
      <span>بررسی‌شده برای انتشار</span>
    </p>
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
  /// Discovery lists link the city to its clean index page; account and admin
  /// lists keep plain text so a management page never offers a detour.
  cityLink?: boolean
}>(), {
  imagePriority: false,
  cityLink: false,
})

const cityHref = computed(() =>
  props.listing.city_slug ? `/city/${props.listing.city_slug}` : '',
)

const imageSizes = LISTING_CARD_IMAGE_SIZES
const jpegSrcsetAttributes = computed(() => (
  props.listing.cover_srcset
    ? { srcset: props.listing.cover_srcset, sizes: imageSizes }
    : {}
))
const imageFailed = ref(false)
const imageElement = ref<HTMLImageElement | null>(null)
watch(() => props.listing.public_id, () => {
  imageFailed.value = false
})

function checkHydratedImage(): void {
  const image = imageElement.value
  // An SSR image can fail before Vue attaches its error listener.
  if (image?.complete && image.naturalWidth === 0) markImageFailed()
}
onMounted(checkHydratedImage)

function markImageFailed(): void {
  imageFailed.value = true
}
</script>
