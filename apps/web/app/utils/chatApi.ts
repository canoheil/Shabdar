/**
 * The single client for private chat traffic, kept in its own module so public
 * route chunks never pay for it.
 *
 * `retry: 0` is deliberate: every chat screen owns an explicit retry action and
 * its own polling cadence, so an implicit ofetch retry would duplicate polls,
 * spend a second slice of the server's rate allowance, and hide a failure the
 * participant must see before acting.
 */
export const apiFetch = $fetch.create({ retry: 0, cache: 'no-store' })
