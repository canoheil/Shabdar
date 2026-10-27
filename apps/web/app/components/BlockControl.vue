<template>
  <div v-if="viewerPublicId !== targetPublicId" class="block-control">
    <AppButton
      variant="secondary"
      :loading="pending"
      @click="mutate"
    >
      {{ pending ? 'در حال به‌روزرسانی…' : blocked ? 'رفع مسدودسازی' : 'مسدود کردن نویسنده' }}
    </AppButton>
    <AppAlert v-if="error" ref="errorAlert" tone="error">{{ error }}</AppAlert>
    <AppButton v-if="error" variant="secondary" :disabled="pending" @click="mutate">
      تلاش دوباره
    </AppButton>
    <p class="meta" role="status" aria-live="polite" aria-atomic="true">{{ notice }}</p>
  </div>
</template>

<script setup lang="ts">
const props = withDefaults(defineProps<{
  targetPublicId: string
  viewerPublicId: string
  initiallyBlocked?: boolean
}>(), {
  initiallyBlocked: false,
})

const emit = defineEmits<{
  confirmed: [blocked: boolean]
  expired: []
}>()

const blocked = ref(props.initiallyBlocked)
const pending = ref(false)
const error = ref('')
const notice = ref('')
const errorAlert = ref<{ focus: () => void } | null>(null)

function requestStatus(requestError: unknown): number | undefined {
  const candidate = requestError as {
    status?: unknown
    statusCode?: unknown
    response?: { status?: unknown }
  }
  for (const value of [candidate?.status, candidate?.statusCode, candidate?.response?.status]) {
    if (typeof value === 'number') return value
  }
  return undefined
}

async function mutate(): Promise<void> {
  if (pending.value) return
  pending.value = true
  error.value = ''
  notice.value = ''
  try {
    const response = await $fetch<{ blocked: boolean }>(
      `/api/v1/me/blocks/${encodeURIComponent(props.targetPublicId)}`,
      { method: blocked.value ? 'DELETE' : 'PUT' },
    )
    blocked.value = response.blocked
    notice.value = response.blocked ? 'کاربر مسدود شد.' : 'مسدودسازی برداشته شد.'
    emit('confirmed', response.blocked)
  } catch (requestError) {
    if (requestStatus(requestError) === 401) {
      emit('expired')
    } else {
      error.value = blocked.value
        ? 'رفع مسدودسازی انجام نشد؛ دوباره تلاش کنید.'
        : 'مسدودسازی انجام نشد؛ دوباره تلاش کنید.'
      await nextTick()
      errorAlert.value?.focus()
    }
  } finally {
    pending.value = false
  }
}
</script>

<style scoped>
.block-control {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.block-control p {
  flex-basis: 100%;
  margin: 0;
}
</style>
