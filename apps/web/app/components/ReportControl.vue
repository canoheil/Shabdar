<template>
  <div ref="root" class="report-control" :data-report-control="controlKey">
    <NuxtLink v-if="anonymous" to="/login" class="report-control__login">ورود برای گزارش</NuxtLink>
    <template v-else>
      <AppButton
        variant="secondary"
        :aria-expanded="open ? 'true' : 'false'"
        :aria-controls="open ? panelId : undefined"
        @click="toggle"
      >
        {{ buttonLabel }}
      </AppButton>
      <ReportForm
        v-if="open"
        :target-type="targetType"
        :target-id="targetId"
        :panel-id="panelId"
        :reported="reported"
        @close="close"
        @submitted="reported = true"
        @busy="busy = $event"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  targetType: 'listing' | 'comment' | 'user'
  targetId: string
  controlKey: 'listing' | 'seller' | 'comment' | 'author'
  buttonLabel: string
  anonymous?: boolean
}>(), {
  anonymous: false,
})

const instanceId = useId()
const panelId = `report-${instanceId.replaceAll(':', '')}`
const root = ref<HTMLElement | null>(null)
const open = ref(false)
const reported = ref(false)
const busy = ref(false)

// The panel is authenticated-only and opens on demand: anonymous visitors get
// only the login link above, so the form never enters their route payload.
const ReportForm = defineAsyncComponent(() => import('~/components/ReportForm.vue'))

async function toggle(): Promise<void> {
  if (open.value) {
    await close()
    return
  }
  open.value = true
}

async function close(): Promise<void> {
  // A submit in flight keeps the panel open, exactly as before the split.
  if (busy.value) return
  open.value = false
  await nextTick()
  root.value?.querySelector<HTMLButtonElement>('.app-button')?.focus()
}
</script>

<style scoped>
.report-control {
  min-inline-size: 0;
}

.report-control__login {
  display: inline-flex;
  align-items: center;
  min-block-size: 44px;
}
</style>
