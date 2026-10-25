<template>
  <NuxtLayout>
    <section class="error-page" :data-status="status">
      <BrandMascot v-if="status === 404" pose="search" class="error-page__mascot" />
      <p class="error-page__status">{{ status }}</p>
      <h1>{{ title }}</h1>
      <p class="error-page__description">{{ description }}</p>
      <div class="error-page__actions">
        <AppButton @click="goHome">بازگشت به خانه</AppButton>
        <AppButton v-if="status === 404" variant="secondary" @click="goSearch">
          جست‌وجو در آگهی‌ها
        </AppButton>
        <AppButton v-else variant="secondary" @click="retry">تلاش دوباره</AppButton>
      </div>
    </section>
  </NuxtLayout>
</template>

<script setup lang="ts">
const props = defineProps<{
  error: {
    statusCode: number
  }
}>()

const route = useRoute()
const status = computed(() => props.error.statusCode)
const title = computed(() => {
  if (status.value === 404) return 'صفحه‌ای که دنبال آن بودید پیدا نشد.'
  if (status.value === 410) return 'این محتوا دیگر در دسترس نیست.'
  return 'مشکلی پیش آمد؛ دوباره تلاش کنید.'
})
const description = computed(() => {
  if (status.value === 404) return 'ممکن است آدرس تغییر کرده باشد؛ از خانه یا جست‌وجو ادامه دهید.'
  if (status.value === 410) return 'می‌توانید به خانه برگردید یا آگهی‌های دیگر را جست‌وجو کنید.'
  return 'اطلاعات بیشتری از خطای سرور نمایش داده نمی‌شود. اتصال خود را بررسی و دوباره تلاش کنید.'
})

async function goHome() {
  await clearError({ redirect: '/' })
}

async function goSearch() {
  await clearError({ redirect: '/#search' })
}

async function retry() {
  await clearError({ redirect: route.fullPath })
}

useHead(() => ({ title: `${status.value} | شبدر` }))
</script>
