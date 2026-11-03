import type { ComputedRef, Ref } from 'vue'
import type { ApiCity } from '~/types'

interface ProvinceCitiesResult {
  province: string
  data: ApiCity[]
}

export interface ProvinceCitiesState {
  cities: ComputedRef<ApiCity[]>
  pending: ComputedRef<boolean>
  error: ComputedRef<boolean>
  refresh: () => Promise<void>
}

/** Load only the selected province's cities and keep SSR/client state aligned. */
export async function useProvinceCities(
  province: Ref<string>,
  key: string,
): Promise<ProvinceCitiesState> {
  const requestFetch = useRequestFetch()
  const { data, pending, error, refresh } = await useAsyncData<ProvinceCitiesResult>(
    key,
    async () => {
      const selected = province.value
      if (!selected) return { province: '', data: [] }
      const response = await requestFetch<{ data: ApiCity[] }>(
        `/api/v1/locations/provinces/${encodeURIComponent(selected)}/cities`,
      )
      return { province: selected, data: response.data }
    },
    {
      default: () => ({ province: '', data: [] }),
      watch: [province],
    },
  )

  return {
    // A previous province's options are never offered under a new selection.
    cities: computed(() => data.value.province === province.value ? data.value.data : []),
    pending: computed(() => pending.value),
    error: computed(() => Boolean(error.value)),
    refresh,
  }
}
