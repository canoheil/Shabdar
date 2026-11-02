<template>
  <button
    class="app-button btn"
    :class="`btn--${variant}`"
    :type="type"
    :disabled="disabled || loading"
    :aria-busy="loading ? 'true' : undefined"
  >
    <span v-if="loading" class="app-button__spinner" aria-hidden="true" />
    <slot />
  </button>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  variant?: 'primary' | 'secondary' | 'danger'
  type?: 'button' | 'submit' | 'reset'
  loading?: boolean
  disabled?: boolean
}>(), {
  variant: 'primary',
  type: 'button',
  loading: false,
  disabled: false,
})
</script>
