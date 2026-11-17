<template>
  <svg class="app-icon" :width="size" :height="size" aria-hidden="true" focusable="false">
    <use :href="`${sprite}#${name}`" />
  </svg>
</template>

<script setup lang="ts">
import spriteUrl from '~/assets/icons/sprite.svg?no-inline'
import type { AppIconName } from '~/utils/icons'

// The client build resolves the asset to an absolute URL and the server to a
// root-relative one. Strip the origin so both render the same attribute.
const sprite = spriteUrl.replace(/^https?:\/\/[^/]+/, '')

withDefaults(defineProps<{ name: AppIconName; size?: number }>(), { size: 20 })
</script>
