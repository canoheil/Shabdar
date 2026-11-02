<template>
  <section ref="root" class="listing-gallery" aria-label="تصاویر آگهی">
    <div
      class="listing-gallery__main"
      @pointerdown="startSwipe"
      @pointerup="finishSwipe"
      @pointercancel="cancelSwipe"
    >
      <picture
        v-if="selectedImage && !mainImageFailed"
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
      <div v-else class="listing-gallery__main-placeholder">
        <span class="listing-gallery__placeholder-mark" aria-hidden="true" />
        <span>{{ selectedImage ? 'تصویر در دسترس نیست' : 'تصویری برای این آگهی ثبت نشده است.' }}</span>
      </div>
      <button
        v-if="images.length > 1"
        type="button"
        class="listing-gallery__nav listing-gallery__nav--previous"
        aria-label="تصویر قبلی"
        :disabled="selectedIndex === 0"
        @click="moveImage(-1)"
      >
        قبلی
      </button>
      <button
        v-if="images.length > 1"
        type="button"
        class="listing-gallery__nav listing-gallery__nav--next"
        aria-label="تصویر بعدی"
        :disabled="selectedIndex === images.length - 1"
        @click="moveImage(1)"
      >
        بعدی
      </button>
      <span v-if="images.length" class="listing-gallery__counter" aria-hidden="true">
        {{ formatNumber(selectedIndex + 1) }} / {{ formatNumber(images.length) }}
      </span>
    </div>
    <p v-if="images.length" class="sr-only" role="status">
      تصویر {{ formatNumber(selectedIndex + 1) }} از {{ formatNumber(images.length) }}
    </p>
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

function moveImage(offset: number): void {
  selectedIndex.value = Math.max(0, Math.min(
    props.images.length - 1,
    selectedIndex.value + offset,
  ))
}

let swipeStart: { x: number; y: number } | null = null
function startSwipe(event: PointerEvent): void {
  if (props.images.length < 2 || !event.isPrimary || event.pointerType === 'mouse') return
  if ((event.target as Element | null)?.closest?.('button')) return
  swipeStart = { x: event.clientX, y: event.clientY }
}
function cancelSwipe(): void {
  swipeStart = null
}
function finishSwipe(event: PointerEvent): void {
  if (!swipeStart || !event.isPrimary) return
  const deltaX = event.clientX - swipeStart.x
  const deltaY = event.clientY - swipeStart.y
  swipeStart = null
  if (Math.abs(deltaX) < Math.max(48, Math.abs(deltaY) * 1.2)) return
  // In the RTL gallery, a rightward swipe advances and a leftward swipe goes back.
  moveImage(deltaX > 0 ? 1 : -1)
}

function markImageFailed(index: number): void {
  failedImages.value = new Set(failedImages.value).add(index)
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
