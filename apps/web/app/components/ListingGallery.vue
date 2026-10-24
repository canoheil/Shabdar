<template>
  <section ref="root" class="listing-gallery" aria-label="تصاویر آگهی">
    <div class="listing-gallery__main">
      <img
        v-if="selectedImage && !mainImageFailed"
        :key="selectedImage.url"
        class="listing-gallery__main-image"
        :src="selectedImage.url"
        :alt="mainImageAlt"
        width="1200"
        height="900"
        loading="eager"
        fetchpriority="high"
        :data-index="selectedIndex"
        @error="markImageFailed(selectedIndex)"
      >
      <div v-else class="listing-gallery__main-placeholder">
        {{ selectedImage ? 'تصویر در دسترس نیست' : 'تصویری برای این آگهی ثبت نشده است.' }}
      </div>
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
        <img
          v-if="!failedImages.has(index)"
          class="listing-gallery__thumbnail-image"
          :src="image.url"
          alt=""
          width="160"
          height="120"
          loading="lazy"
          :data-index="index"
          @error="markImageFailed(index)"
        >
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

<style scoped>
.listing-gallery {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-inline-size: 0;
}

.listing-gallery__main {
  display: flex;
  align-items: center;
  justify-content: center;
  inline-size: 100%;
  aspect-ratio: 4 / 3;
  overflow: hidden;
  color: var(--color-text-muted);
  text-align: center;
  background: var(--color-surface-muted);
  border-radius: var(--radius-card);
}

.listing-gallery__main-image {
  display: block;
  inline-size: 100%;
  block-size: 100%;
  object-fit: cover;
}

.listing-gallery__main-placeholder {
  padding: var(--space-4);
}

.listing-gallery__thumbnails {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-2);
}

.listing-gallery__thumbnail {
  display: flex;
  align-items: center;
  justify-content: center;
  min-inline-size: 44px;
  min-block-size: 44px;
  aspect-ratio: 4 / 3;
  overflow: hidden;
  padding: 2px;
  color: var(--color-text-muted);
  background: var(--color-surface-muted);
  border: 2px solid transparent;
  border-radius: var(--radius-control);
  cursor: pointer;
}

.listing-gallery__thumbnail--selected {
  border-color: var(--color-primary);
}

.listing-gallery__thumbnail-image {
  display: block;
  inline-size: 100%;
  block-size: 100%;
  object-fit: cover;
  border-radius: calc(var(--radius-control) - 4px);
}

.listing-gallery__thumbnail-placeholder {
  font-size: 0.75rem;
}
</style>
