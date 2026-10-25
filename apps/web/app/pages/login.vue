<template>
  <section class="login">
    <div class="login__panel">
      <header class="login__header">
        <BrandMascot pose="full" :decorative="true" class="login__mascot" />
        <div>
          <h1>{{ purpose === 'login' ? 'ورود به شبدر' : 'ثبت‌نام در شبدر' }}</h1>
          <p>با شمارهٔ موبایل و کد یک‌بارمصرف ادامه دهید.</p>
        </div>
      </header>

      <div class="login__purpose" role="group" aria-label="نوع ورود">
        <AppButton
          class="login__purpose-button"
          :variant="purpose === 'login' ? 'primary' : 'secondary'"
          :aria-pressed="purpose === 'login'"
          @click="selectPurpose('login')"
        >
          ورود
        </AppButton>
        <AppButton
          class="login__purpose-button"
          :variant="purpose === 'register' ? 'primary' : 'secondary'"
          :aria-pressed="purpose === 'register'"
          @click="selectPurpose('register')"
        >
          ثبت‌نام
        </AppButton>
      </div>

      <AppAlert
        v-if="errorMessage"
        id="login-error"
        ref="errorBanner"
        tone="error"
      >
        {{ errorMessage }}
      </AppAlert>

      <form v-if="step === 'phone'" class="login__form" @submit.prevent="requestCode">
        <AppField id="login-phone" label="شمارهٔ موبایل" help="شماره را با پیش‌شمارهٔ کشور وارد کنید.">
          <template #default="{ id, describedBy }">
            <input
              :id="id"
              v-model="phone"
              class="field__input login__input"
              type="tel"
              inputmode="tel"
              autocomplete="tel"
              dir="ltr"
              :disabled="loading"
              :aria-describedby="[describedBy, errorMessage ? 'login-error' : ''].filter(Boolean).join(' ') || undefined"
              :aria-invalid="errorMessage ? 'true' : undefined"
              required
            >
          </template>
        </AppField>
        <AppButton type="submit" :loading="loading">ارسال کد</AppButton>
      </form>

      <form v-else class="login__form" @submit.prevent="verifyCode">
        <p class="login__phone">
          <span>کد به شمارهٔ <span dir="ltr">{{ phone }}</span> ارسال شد.</span>
          <button type="button" class="login__edit" :disabled="loading" @click="backToPhone">
            ویرایش شماره
          </button>
        </p>
        <AppField id="login-code" label="کد تأیید" help="کد شش‌رقمی ارسال‌شده را وارد کنید.">
          <template #default="{ id, describedBy }">
            <input
              :id="id"
              v-model="code"
              class="field__input login__input"
              type="text"
              inputmode="numeric"
              autocomplete="one-time-code"
              maxlength="6"
              dir="ltr"
              :disabled="loading"
              :aria-describedby="[describedBy, errorMessage ? 'login-error' : ''].filter(Boolean).join(' ') || undefined"
              :aria-invalid="errorMessage ? 'true' : undefined"
              required
            >
          </template>
        </AppField>
        <AppButton type="submit" :loading="loading">تأیید</AppButton>
        <AppButton
          variant="secondary"
          :loading="loading"
          :disabled="resendIn > 0"
          @click="requestCode"
        >
          {{ resendIn > 0 ? `ارسال مجدد کد (${resendIn} ثانیه)` : 'ارسال مجدد کد' }}
        </AppButton>
      </form>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { ApiErrorBody } from '~/types'

type Step = 'phone' | 'code'
type Purpose = 'login' | 'register'

const { refresh: refreshAuth } = await useAuth()

const purpose = ref<Purpose>('login')
const step = ref<Step>('phone')
const phone = ref('')
const code = ref('')
const loading = ref(false)
const errorMessage = ref<string | null>(null)
const challengeId = ref('')
const resendIn = ref(0)
const errorBanner = ref<{ focus: () => void } | null>(null)
let resendTimer: ReturnType<typeof setInterval> | null = null

onUnmounted(() => stopResendTimer())

function stopResendTimer() {
  if (resendTimer !== null) {
    clearInterval(resendTimer)
    resendTimer = null
  }
}

function startResendTimer(seconds: number) {
  stopResendTimer()
  resendIn.value = seconds
  resendTimer = setInterval(() => {
    if (resendIn.value <= 1) {
      stopResendTimer()
      resendIn.value = 0
    } else {
      resendIn.value -= 1
    }
  }, 1000)
}

function selectPurpose(next: Purpose) {
  purpose.value = next
  backToPhone()
}

function backToPhone() {
  stopResendTimer()
  step.value = 'phone'
  code.value = ''
  errorMessage.value = null
}

function persianError(code: string): string {
  switch (code) {
    case 'invalid_phone':
      return 'شمارهٔ موبایل معتبر نیست.'
    case 'rate_limited':
      return 'درخواست‌های زیاد؛ کمی صبر کنید.'
    case 'invalid_code':
      return 'کد واردشده نادرست است.'
    case 'challenge_expired':
      return 'کد منقضی شده است؛ کد جدید بخواهید.'
    case 'challenge_locked':
      return 'تعداد تلاش‌ها بیش از حد مجاز است؛ کد جدید بخواهید.'
    case 'no_account':
      return 'حسابی با این شماره وجود ندارد.'
    default:
      return 'خطایی رخ داد؛ دوباره تلاش کنید.'
  }
}

function showError(err: unknown) {
  const body = (err as { data?: ApiErrorBody }).data
  const code = body?.error?.code ?? ''
  if (code === 'rate_limited' && resendIn.value > 0) {
    errorMessage.value = `درخواست‌های زیاد؛ ${resendIn.value} ثانیهٔ دیگر دوباره تلاش کنید.`
  } else {
    errorMessage.value = persianError(code)
  }
  void nextTick(() => errorBanner.value?.focus())
}

async function requestCode() {
  loading.value = true
  errorMessage.value = null
  try {
    const res = await $fetch<{
      challenge_id: string
      expires_in_seconds: number
      resend_after_seconds: number
    }>('/api/v1/auth/otp/request', {
      method: 'POST',
      body: { phone: phone.value, purpose: purpose.value },
    })
    challengeId.value = res.challenge_id
    step.value = 'code'
    code.value = ''
    startResendTimer(res.resend_after_seconds)
  } catch (err) {
    showError(err)
  } finally {
    loading.value = false
  }
}

async function verifyCode() {
  loading.value = true
  errorMessage.value = null
  try {
    await $fetch('/api/v1/auth/otp/verify', {
      method: 'POST',
      body: { challenge_id: challengeId.value, code: code.value },
    })
    await refreshAuth()
    await navigateTo('/')
  } catch (err) {
    showError(err)
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.login {
  display: flex;
  justify-content: center;
  inline-size: 100%;
}

.login__panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  inline-size: min(100%, 28rem);
  min-block-size: 580px;
  padding: var(--space-6);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-dialog);
}

.login__header {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.login__mascot {
  flex: 0 0 80px;
  inline-size: 80px;
  block-size: 80px;
}

.login__header h1,
.login__header p,
.login__phone {
  margin: 0;
}

.login__header h1 {
  font-size: 1.5rem;
  line-height: 1.5;
}

.login__header p {
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.login__purpose {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.login__form {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-4);
}

.login__input {
  text-align: start;
}

.login__phone {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  color: var(--color-text-muted);
}

.login__edit {
  min-block-size: 44px;
  padding-inline: var(--space-2);
  font: inherit;
  color: var(--color-primary);
  background: transparent;
  border: 0;
  border-radius: var(--radius-control);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 0.2em;
}

@media (max-width: 420px) {
  .login__panel {
    padding: var(--space-4);
  }

  .login__header {
    align-items: flex-start;
  }

  .login__mascot {
    flex-basis: 64px;
    inline-size: 64px;
    block-size: 64px;
  }
}
</style>
