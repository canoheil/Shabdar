<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
import arabicFontUrl from '@fontsource-variable/vazirmatn/files/vazirmatn-arabic-wght-normal.woff2?url'
import latinFontUrl from '@fontsource-variable/vazirmatn/files/vazirmatn-latin-wght-normal.woff2?url'

// Preload the same two Vite asset URLs used by the variable-font stylesheet.
// This retains the required swap behavior without a late second LCP layout.
useHead({
  link: [arabicFontUrl, latinFontUrl].map((href, index) => ({
    key: `vazirmatn-${index}`,
    rel: 'preload',
    as: 'font',
    type: 'font/woff2',
    href,
    crossorigin: 'anonymous',
  })),
})
</script>
