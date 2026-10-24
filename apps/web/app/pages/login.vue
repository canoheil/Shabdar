<!--
  OTP login (plan Task 7 — single component, local step state, no store).

  States and their expected rendering (asserted in the WP 2 phone gate):
  - step=phone, idle   : phone field + «ارسال کد», purpose toggle above.
  - step=phone, loading: fields + button disabled.
  - step=phone, error  : error banner (role=alert, focused) bound to error.code
                         — invalid_phone / rate_limited (with the wait) / generic.
  - step=code          : entered number + «ویرایش شماره» (back to phone, number
                         preserved), code field + «تأیید» + resend button with
                         a live countdown from resend_after_seconds.
  - step=code, error   : banner bound to error.code — invalid_code /
                         challenge_expired / challenge_locked / no_account /
                         rate_limited / generic.
  - verify 200         : navigateTo('/').
-->
<template>
  <section class="login">
    <h1>ورود به شبدر</h1>

    <!-- Purpose toggle: ورود / ثبت‌نام (PRODUCT.md §3 — separate flows). -->
    <div class="login__purpose" role="group" aria-label="نوع ورود">
      <button
        type="button"
        class="btn"
        :aria-pressed="purpose === 'login'"
        @click="selectPurpose('login')"
      >
        ورود
      </button>
      <button
        type="button"
        class="btn"
        :aria-pressed="purpose === 'register'"
        @click="selectPurpose('register')"
      >
        ثبت‌نام
      </button>
    </div>

    <!-- Error banner: bound to error.code, focused after a failed submit. -->
    <p
      v-if="errorMessage"
      id="login-error"
      ref="errorBanner"
      class="login__error"
      role="alert"
      tabindex="-1"
    >
      {{ errorMessage }}
    </p>

    <!-- Step: phone -->
    <form v-if="step === 'phone'" class="login__form" @submit.prevent="requestCode">
      <label class="field">
        <span>شمارهٔ موبایل</span>
        <input
          v-model="phone"
          class="field__input"
          type="tel"
          inputmode="tel"
          autocomplete="tel"
          dir="ltr"
          :disabled="loading"
          :aria-describedby="errorMessage ? 'login-error' : undefined"
          required
        >
      </label>
      <button type="submit" class="btn" :disabled="loading">
        ارسال کد
      </button>
    </form>

    <!-- Step: code -->
    <form v-else class="login__form" @submit.prevent="verifyCode">
      <p class="login__phone">
        کد به شمارهٔ <span dir="ltr">{{ phone }}</span> ارسال شد.
        <button type="button" class="login__edit" @click="backToPhone">
          ویرایش شماره
        </button>
      </p>
      <label class="field">
        <span>کد تأیید</span>
        <input
          v-model="code"
          class="field__input"
          type="text"
          inputmode="numeric"
          autocomplete="one-time-code"
          dir="ltr"
          :disabled="loading"
          :aria-describedby="errorMessage ? 'login-error' : undefined"
          required
        >
      </label>
      <button type="submit" class="btn" :disabled="loading">
        تأیید
      </button>
      <button
        type="button"
        class="btn btn--secondary"
        :disabled="loading || resendIn > 0"
        @click="requestCode"
      >
        {{ resendIn > 0 ? `ارسال مجدد کد (${resendIn} ثانیه)` : 'ارسال مجدد کد' }}
      </button>
    </form>
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
const errorBanner = ref<HTMLElement | null>(null)
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
  // A new purpose needs a fresh challenge: back to the phone step.
  backToPhone()
}

function backToPhone() {
  stopResendTimer()
  step.value = 'phone'
  code.value = ''
  errorMessage.value = null
}

// error.code → Persian message (frozen codes; unknown → generic).
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
    // rate_limited: show the wait when a countdown is known.
    errorMessage.value = `درخواست‌های زیاد؛ ${resendIn.value} ثانیهٔ دیگر دوباره تلاش کنید.`
  } else {
    errorMessage.value = persianError(code)
  }
  // Move focus to the banner (ui-ux-pro-max: focusable error summary).
  nextTick(() => errorBanner.value?.focus())
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
      // Phone sent as typed — the API normalizes Persian digits (PRODUCT.md §5-D).
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
    // Refresh the shared session before any page renders authenticated UI.
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
  flex-direction: column;
  gap: 1rem;
}

.login__purpose {
  display: flex;
  gap: 0.5rem;
}

.login__form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  max-inline-size: 22rem;
}

.login__phone {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  margin: 0;
}

.login__edit {
  min-block-size: 44px;
  padding-inline: 0.5rem;
  font: inherit;
  color: #2b6cb0;
  background: none;
  border: none;
  cursor: pointer;
  text-decoration: underline;
}

.login__error {
  margin: 0;
  padding: 0.75rem 1rem;
  color: #9b2c2c;
  background: #fff5f5;
  border: 1px solid #feb2b2;
  border-radius: 8px;
}

.login__error:focus {
  outline: 2px solid #9b2c2c;
  outline-offset: 2px;
}
</style>
