import type { ComputedRef } from 'vue'
import type { ApiUser } from '~/types'

export type AuthStatus = 'pending' | 'authenticated' | 'anonymous' | 'unavailable'

export interface AuthState {
  user: ComputedRef<ApiUser | null>
  status: ComputedRef<AuthStatus>
  pending: ComputedRef<boolean>
  refresh: () => Promise<void>
}

function errorStatus(error: unknown): number | undefined {
  const candidate = error as {
    status?: unknown
    statusCode?: unknown
    response?: { status?: unknown }
  } | null
  for (const value of [candidate?.status, candidate?.statusCode, candidate?.response?.status]) {
    if (typeof value === 'number') return value
  }
  return undefined
}

export async function useAuth(): Promise<AuthState> {
  const {
    data,
    pending: fetchPending,
    error,
    refresh: refreshFetch,
  } = await useFetch<{ user: ApiUser }>('/api/v1/me', {
    key: 'auth:me',
    headers: useRequestHeaders(['cookie']),
  })

  const user = computed(() => data.value?.user ?? null)
  const pending = computed(() => fetchPending.value)
  const status = computed<AuthStatus>(() => {
    if (fetchPending.value) return 'pending'
    if (error.value) return errorStatus(error.value) === 401 ? 'anonymous' : 'unavailable'
    if (data.value?.user) return 'authenticated'
    return 'unavailable'
  })

  return {
    user,
    status,
    pending,
    refresh: async () => {
      await refreshFetch()
    },
  }
}
