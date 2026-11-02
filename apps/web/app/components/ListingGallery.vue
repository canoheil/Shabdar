<template>
  <section ref="root" class="listing-gallery" aria-label="تصاویر آگهی">
    <div class="listing-gallery__main">
      <picture
        v-if="selectedImage && !mainImageFailed && selectedImage.webp_srcset"
        :key="selectedImage.url"
        class="listing-gallery__main-picture"
      >
        <source
          v-if="selectedImage.webp_srcset"
          type="image/webp"
          :srcset="selectedImage.webp_srcset"
          sizes="(min-width: 1440px) 720px, (min-width: 1024px) 58vw, (min-width: 900px) 60vw, calc(100vw - 32px)"
        >
        <img
          class="listing-gallery__main-image"
          :src="selectedImage.url"
          v-bind="selectedJpegSrcsetAttributes"
          :alt="mainImageAlt"
          width="1200"
          height="900"
          loading="eager"
          fetchpriority="high"
          decoding="async"
          :data-index="selectedIndex"
          @error="markImageFailed(selectedIndex)"
        >
      </picture>
      <img
        v-else-if="selectedImage && !mainImageFailed"
        class="listing-gallery__main-image"
        :src="selectedImage.url"
        v-bind="selectedJpegSrcsetAttributes"
        :alt="mainImageAlt"
        width="1200"
        height="900"
        loading="eager"
        fetchpriority="high"
        decoding="async"
        :data-index="selectedIndex"
        @error="markImageFailed(selectedIndex)"
      >
      <div v-else class="listing-gallery__main-placeholder">
        <span class="listing-gallery__placeholder-mark" aria-hidden="true" />
        <span>{{ selectedImage ? 'تصویر در دسترس نیست' : 'تصویری برای این آگهی ثبت نشده است.' }}</span>
      </div>
      <span v-if="images.length" class="listing-gallery__counter" aria-hidden="true">
        {{ formatNumber(selectedIndex + 1) }} / {{ formatNumber(images.length) }}
      </span>
    </div>
    <div v-if="images.length" class="listing-gallery__thumbnails" role="group" aria-label="انتخاب تصویر">
      <button
        v-for="(image, index) in images"
        :key="`${image.url}-${index}`"
        type="button"
        class="listing-gallery__thumbnail"
        :class="{ 'listing-gallery__thumbnail--selected': selectedIndex === index }"
        :aria-pressed="selectedIndex === index"
        :aria-label="`تصویر ${index + 1} از ${images.length}`"
        @click="selectedIndex = index"
      >
        <picture v-if="!failedImages.has(index)" class="listing-gallery__thumbnail-picture">
          <source
            v-if="image.webp_srcset"
            type="image/webp"
            v-bind="listingThumbnailAttributes(image.webp_srcset)"
          >
          <img
            class="listing-gallery__thumbnail-image"
            :src="image.url"
            v-bind="listingThumbnailAttributes(image.srcset ?? '')"
            alt=""
            width="160"
            height="120"
            loading="lazy"
            fetchpriority="low"
            decoding="async"
            :data-index="index"
            @error="markImageFailed(index)"
          >
        </picture>
        <span v-else class="listing-gallery__thumbnail-placeholder" aria-hidden="true">ناموجود</span>
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { ApiListingImage } from '~/types'

const props = defineProps<{
  images: ApiListingImage[]
  listingTitle: string
}>()

const selectedIndex = ref(0)
const failedImages = ref<Set<number>>(new Set())
const root = ref<HTMLElement | null>(null)

const selectedImage = computed(() => props.images[selectedIndex.value])
const mainImageFailed = computed(() => failedImages.value.has(selectedIndex.value))
const mainImageAlt = computed(() => (
  selectedImage.value?.alt_text ?? `${props.listingTitle} — تصویر ${selectedIndex.value + 1}`
))
const selectedJpegSrcsetAttributes = computed(() => (
  selectedImage.value?.srcset
    ? { srcset: selectedImage.value.srcset, sizes: LISTING_GALLERY_IMAGE_SIZES }
    : {}
))

function markImageFailed(index: number): void {
  const next = new Set(failedImages.value)
  next.add(index)
  failedImages.value = next
}

function checkHydratedImages(): void {
  root.value?.querySelectorAll<HTMLImageElement>('img[data-index]').forEach((image) => {
    if (image.complete && image.naturalWidth === 0) {
      markImageFailed(Number(image.dataset.index))
    }
  })
}

watch(() => props.images, (images) => {
  selectedIndex.value = Math.min(selectedIndex.value, Math.max(0, images.length - 1))
  failedImages.value = new Set()
  void nextTick(checkHydratedImages)
})
watch(selectedIndex, () => {
  void nextTick(checkHydratedImages)
})
onMounted(checkHydratedImages)
</script>
