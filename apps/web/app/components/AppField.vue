<template>
  <div class="app-field field">
    <label class="app-field__label" :for="id">{{ label }}</label>
    <slot :id="id" :describedBy="describedBy" :ariaInvalid="ariaInvalid" />
    <p v-if="help" :id="`${id}-help`" class="app-field__help">{{ help }}</p>
    <p v-if="error" :id="`${id}-error`" class="app-field__error">{{ error }}</p>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  id: string
  label: string
  help?: string
  error?: string
}>()

const describedBy = computed(() => {
  const ids = [
    props.help ? `${props.id}-help` : '',
    props.error ? `${props.id}-error` : '',
  ].filter(Boolean)
  return ids.length ? ids.join(' ') : undefined
})
const ariaInvalid = computed<'true' | undefined>(() => props.error ? 'true' : undefined)
</script>

<style scoped>
.app-field__label {
  font-weight: 600;
}

.app-field__help,
.app-field__error {
  margin: 0;
  font-size: 0.875rem;
}

.app-field__help {
  color: var(--color-text-muted);
}

.app-field__error {
  color: var(--color-error);
}
</style>
