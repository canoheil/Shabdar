<template>
  <section class="advertising-page" aria-labelledby="advertising-title">
    <header class="advertising-page__intro">
      <p class="advertising-page__kicker">تبلیغ کسب‌وکار محلی</p>
      <h1 id="advertising-title">در شبدر دیده شوید</h1>
      <p>
        اگر کسب‌وکار یا خدمت محلی دارید، درخواست خود را ثبت کنید. تیم مدیریت شبدر
        جایگاه و متن پیشنهادی را با شما هماهنگ می‌کند.
      </p>
      <ul class="advertising-page__steps">
        <li>مشخصات تماس و درخواست خود را بنویسید.</li>
        <li>درخواست در پنل مدیریت ثبت و بررسی می‌شود.</li>
        <li>برای هماهنگی بعدی با شمارهٔ تماس شما پیگیری می‌کنیم.</li>
      </ul>
      <p class="advertising-page__privacy">
        ثبت درخواست نیاز به ورود ندارد. شمارهٔ تماس فقط در بخش خصوصی مدیریت نمایش داده می‌شود.
      </p>
    </header>

    <div class="advertising-page__form-panel">
      <AppAlert v-if="receipt" ref="successAlert" tone="success">
        <div class="advertising-page__success">
          <h2>درخواست شما ثبت شد</h2>
          <p>درخواست برای پیگیری در پنل مدیریت قرار گرفت.</p>
          <AppButton variant="secondary" @click="startAnotherRequest">ثبت درخواست دیگر</AppButton>
        </div>
      </AppAlert>

      <form v-else class="advertising-form" :aria-busy="submitting" @submit.prevent="submitRequest">
        <div>
          <h2>فرم درخواست تبلیغ</h2>
          <p class="meta">فیلدهای ستاره‌دار الزامی هستند.</p>
        </div>
        <AppAlert v-if="formError" tone="error">{{ formError }}</AppAlert>
        <AppField
          id="advertising-contact-name"
          label="نام شما یا کسب‌وکار *"
          :error="fieldErrors.contact_name"
        >
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="contactName"
              class="field__input"
              type="text"
              name="contact_name"
              autocomplete="name"
              required
              maxlength="80"
              :disabled="submitting"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
          </template>
        </AppField>
        <AppField
          id="advertising-phone"
          label="شمارهٔ موبایل برای پیگیری *"
          help="مثال: 09123456789"
          :error="fieldErrors.phone"
        >
          <template #default="{ id, describedBy, ariaInvalid }">
            <input
              :id="id"
              v-model="phone"
              class="field__input"
              type="tel"
              name="phone"
              autocomplete="tel"
              inputmode="tel"
              dir="ltr"
              required
              maxlength="20"
              :disabled="submitting"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            >
          </template>
        </AppField>
        <AppField
          id="advertising-details"
          label="دربارهٔ کسب‌وکار یا جایگاه پیشنهادی"
          help="اختیاری؛ حداکثر ۱۰۰۰ نویسه"
          :error="fieldErrors.details"
        >
          <template #default="{ id, describedBy, ariaInvalid }">
            <textarea
              :id="id"
              v-model="details"
              class="field__input advertising-form__details"
              name="details"
              rows="5"
              maxlength="1000"
              :disabled="submitting"
              :aria-describedby="describedBy"
              :aria-invalid="ariaInvalid"
            />
          </template>
        </AppField>
        <AppButton type="submit" :loading="submitting">
          {{ submitting ? 'در حال ثبت درخواست…' : 'ثبت درخواست تبلیغ' }}
        </AppButton>
      </form>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { ApiAdvertisingRequestReceipt, ApiErrorBody } from '~/types'

const contactName = ref('')
const phone = ref('')
const details = ref('')
const submitting = ref(false)
const formError = ref('')
const fieldErrors = reactive<Record<'contact_name' | 'phone' | 'details', string>>({
  contact_name: '',
  phone: '',
  details: '',
})
const receipt = ref<ApiAdvertisingRequestReceipt['request'] | null>(null)
const successAlert = ref<{ focus: () => void } | null>(null)
// A retry after a lost response uses the same id, so the API returns the
// original receipt instead of creating a duplicate. It is not a viewer id.
const submissionId = useState('advertising-request-submission', () => crypto.randomUUID())

function clearErrors(): void {
  formError.value = ''
  fieldErrors.contact_name = ''
  fieldErrors.phone = ''
  fieldErrors.details = ''
}

async function submitRequest(): Promise<void> {
  if (submitting.value || receipt.value) return
  submitting.value = true
  clearErrors()
  try {
    const confirmed = await $fetch<ApiAdvertisingRequestReceipt>('/api/v1/advertising/requests', {
      method: 'POST',
      body: {
        submission_id: submissionId.value,
        contact_name: contactName.value,
        phone: phone.value,
        details: details.value,
      },
      cache: 'no-store',
    })
    receipt.value = confirmed.request
    await nextTick()
    successAlert.value?.focus()
  } catch (error: unknown) {
    // Inputs and submission id intentionally remain untouched for retry.
    const failure = error as {
      statusCode?: number
      response?: { status?: number }
      data?: ApiErrorBody
    }
    const status = failure.statusCode ?? failure.response?.status ?? 0
    const fields = failure.data?.error?.fields
    if (fields?.contact_name) {
      fieldErrors.contact_name = fields.contact_name === 'too_long'
        ? 'نام نمی‌تواند بیشتر از ۸۰ نویسه باشد.'
        : 'نام را وارد کنید.'
    }
    if (fields?.phone) fieldErrors.phone = 'شمارهٔ موبایل معتبر ایران وارد کنید.'
    if (fields?.details) {
      fieldErrors.details = fields.details === 'too_long'
        ? 'توضیحات نمی‌تواند بیشتر از ۱۰۰۰ نویسه باشد.'
        : 'متن توضیحات معتبر نیست.'
    }
    const hasKnownFieldError = Boolean(fields?.contact_name || fields?.phone || fields?.details)
    if (status === 429) {
      formError.value = 'ارسال درخواست بیش از حد مجاز است؛ کمی بعد دوباره تلاش کنید.'
    } else if (!hasKnownFieldError) {
      formError.value = 'ثبت درخواست انجام نشد؛ اطلاعات فرم حفظ شده است. دوباره تلاش کنید.'
    }
  } finally {
    submitting.value = false
  }
}

function startAnotherRequest(): void {
  contactName.value = ''
  phone.value = ''
  details.value = ''
  receipt.value = null
  submissionId.value = crypto.randomUUID()
  clearErrors()
}

useHead({
  title: 'ثبت تبلیغ در شبدر',
  meta: [{
    name: 'description',
    content: 'ثبت درخواست داخلی برای معرفی کسب‌وکار محلی در جایگاه تبلیغاتی شبدر.',
  }],
})
</script>

<style scoped>
.advertising-page {
  display: grid;
  grid-template-columns: minmax(0, 0.85fr) minmax(20rem, 1.15fr);
  gap: var(--space-8);
  align-items: start;
  max-inline-size: 68rem;
  margin-inline: auto;
}

.advertising-page__intro,
.advertising-page__form-panel {
  padding: var(--space-6);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-panel);
}

.advertising-page__intro h1,
.advertising-page__intro p,
.advertising-form h2,
.advertising-form p,
.advertising-page__success h2,
.advertising-page__success p {
  margin: 0;
}

.advertising-page__intro {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.advertising-page__kicker {
  color: var(--color-primary);
  font-weight: 700;
}

.advertising-page__intro > p:not(.advertising-page__kicker) {
  line-height: 1.9;
}

.advertising-page__steps {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding-inline-start: var(--space-5);
  line-height: 1.8;
}

.advertising-page__privacy {
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.advertising-form,
.advertising-page__success {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--space-4);
}

.advertising-form__details {
  min-block-size: 8rem;
  resize: vertical;
}

@media (max-width: 760px) {
  .advertising-page {
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-4);
  }

  .advertising-page__intro,
  .advertising-page__form-panel {
    padding: var(--space-5);
  }
}
</style>
